#include <errno.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include "native_tui.h"

#ifdef _WIN32
#define WIN32_LEAN_AND_MEAN
#include <windows.h>
#else
#include <poll.h>
#include <sys/ioctl.h>
#include <termios.h>
#include <unistd.h>
#endif

typedef struct {
#ifdef _WIN32
    HANDLE input;
    HANDLE output;
    DWORD input_mode;
    DWORD output_mode;
#else
    struct termios termios;
#endif
    int active;
} CcloverTui;

static size_t utf8_prefix(const uint8_t *text, size_t len, size_t columns) {
    size_t offset = 0;
    size_t count = 0;
    while (offset < len && count < columns) {
        uint8_t byte = text[offset];
        size_t width = 1;
        if ((byte & 0xe0u) == 0xc0u) width = 2;
        else if ((byte & 0xf0u) == 0xe0u) width = 3;
        else if ((byte & 0xf8u) == 0xf0u) width = 4;
        if (offset + width > len) break;
        offset += width;
        count += 1;
    }
    return offset;
}

static void cursor_at(unsigned x, unsigned y) {
    fprintf(stdout, "\x1b[%u;%uH", y + 1, x + 1);
}

static void write_text(unsigned x, unsigned y, unsigned width, CcloverText text) {
    if (width == 0 || text.ptr == NULL || text.len == 0) return;
    cursor_at(x, y);
    size_t bytes = utf8_prefix(text.ptr, text.len, width);
    fwrite(text.ptr, 1, bytes, stdout);
}

static void repeat_utf8(const char *glyph, unsigned count) {
    for (unsigned i = 0; i < count; ++i) fputs(glyph, stdout);
}

static void draw_box(unsigned x, unsigned y, unsigned width, unsigned height, CcloverText title) {
    if (width < 2 || height < 2) return;
    cursor_at(x, y);
    fputs("┌", stdout);
    unsigned title_cols = 0;
    if (title.ptr != NULL && title.len != 0 && width > 3) {
        unsigned max_title = width - 3;
        size_t bytes = utf8_prefix(title.ptr, title.len, max_title);
        fwrite(title.ptr, 1, bytes, stdout);
        title_cols = (unsigned)utf8_prefix(title.ptr, bytes, max_title);
        /* utf8_prefix returns bytes, recompute columns cheaply. */
        title_cols = 0;
        for (size_t i = 0; i < bytes;) {
            uint8_t byte = title.ptr[i];
            i += (byte & 0x80u) == 0 ? 1 : ((byte & 0xe0u) == 0xc0u ? 2 : ((byte & 0xf0u) == 0xe0u ? 3 : 4));
            title_cols++;
        }
    }
    if (width > title_cols + 2) repeat_utf8("─", width - title_cols - 2);
    fputs("┐", stdout);
    for (unsigned row = 1; row + 1 < height; ++row) {
        cursor_at(x, y + row);
        fputs("│", stdout);
        cursor_at(x + width - 1, y + row);
        fputs("│", stdout);
    }
    cursor_at(x, y + height - 1);
    fputs("└", stdout);
    repeat_utf8("─", width - 2);
    fputs("┘", stdout);
}

static void draw_rows(const CcloverPanel *panel, unsigned x, unsigned y, unsigned width, unsigned height, unsigned skip) {
    if (width <= 2 || height <= 2) return;
    unsigned available = height - 2;
    for (unsigned row = skip; row < available && (size_t)(row - skip) < panel->row_count; ++row) {
        write_text(x + 1, y + 1 + row, width - 2, panel->rows[row - skip]);
    }
}

static void draw_history(const CcloverPanel *panel, unsigned x, unsigned y, unsigned width, unsigned height) {
    static const char *levels[] = {"▁", "▂", "▃", "▄", "▅", "▆", "▇", "█"};
    if (width <= 2 || height <= 2 || panel->history_count == 0) return;
    unsigned inner = width - 2;
    unsigned rows = height - 2 < 3 ? height - 2 : 3;
    uint64_t maximum = 1;
    for (size_t i = 0; i < panel->history_count; ++i) {
        if (panel->history[i] > maximum) maximum = panel->history[i];
    }
    size_t count = panel->history_count < inner ? panel->history_count : inner;
    for (size_t column = 0; column < count; ++column) {
        uint64_t scaled = (uint64_t)(((long double)panel->history[column] * rows * 8) / maximum);
        for (unsigned row = 0; row < rows; ++row) {
            unsigned from_bottom = rows - row - 1;
            uint64_t remaining = scaled > (uint64_t)from_bottom * 8
                ? scaled - (uint64_t)from_bottom * 8
                : 0;
            unsigned level = remaining >= 8 ? 7 : (unsigned)remaining;
            if (remaining == 0) continue;
            cursor_at(x + 1 + (unsigned)column, y + 1 + row);
            fputs(levels[level], stdout);
        }
    }
}

static void draw_panel(const CcloverPanel *panel, unsigned x, unsigned y, unsigned width, unsigned height, int sparkline) {
    draw_box(x, y, width, height, panel->title);
    if (sparkline) {
        draw_history(panel, x, y, width, height);
        unsigned row_start = height > 5 ? 3 : 1;
        if (height > row_start + 1) {
            unsigned available = height - row_start - 1;
            for (unsigned row = 0; row < available && (size_t)row < panel->row_count; ++row) {
                write_text(x + 1, y + row_start + row, width - 2, panel->rows[row]);
            }
        }
    } else {
        draw_rows(panel, x, y, width, height, 0);
    }
}

static void terminal_size(unsigned *width, unsigned *height) {
#ifdef _WIN32
    CONSOLE_SCREEN_BUFFER_INFO info;
    HANDLE output = GetStdHandle(STD_OUTPUT_HANDLE);
    if (GetConsoleScreenBufferInfo(output, &info)) {
        *width = (unsigned)(info.srWindow.Right - info.srWindow.Left + 1);
        *height = (unsigned)(info.srWindow.Bottom - info.srWindow.Top + 1);
        return;
    }
#else
    struct winsize size;
    if (ioctl(STDOUT_FILENO, TIOCGWINSZ, &size) == 0 && size.ws_col && size.ws_row) {
        *width = size.ws_col;
        *height = size.ws_row;
        return;
    }
#endif
    *width = 80;
    *height = 24;
}

int cclover_tui_enter(CcloverTui **out) {
    if (out == NULL) return EINVAL;
    CcloverTui *ui = (CcloverTui *)calloc(1, sizeof(*ui));
    if (ui == NULL) return ENOMEM;
#ifdef _WIN32
    ui->input = GetStdHandle(STD_INPUT_HANDLE);
    ui->output = GetStdHandle(STD_OUTPUT_HANDLE);
    if (ui->input == INVALID_HANDLE_VALUE || ui->output == INVALID_HANDLE_VALUE ||
        !GetConsoleMode(ui->input, &ui->input_mode) || !GetConsoleMode(ui->output, &ui->output_mode)) {
        free(ui);
        return EIO;
    }
    DWORD input_mode = ui->input_mode & ~(ENABLE_ECHO_INPUT | ENABLE_LINE_INPUT | ENABLE_PROCESSED_INPUT);
    input_mode |= ENABLE_WINDOW_INPUT;
    DWORD output_mode = ui->output_mode | ENABLE_VIRTUAL_TERMINAL_PROCESSING;
    if (!SetConsoleMode(ui->input, input_mode)) {
        free(ui);
        return EIO;
    }
    if (!SetConsoleMode(ui->output, output_mode)) {
        SetConsoleMode(ui->input, ui->input_mode);
        free(ui);
        return EIO;
    }
#else
    if (!isatty(STDIN_FILENO) || !isatty(STDOUT_FILENO) || tcgetattr(STDIN_FILENO, &ui->termios) != 0) {
        free(ui);
        return errno ? errno : ENOTTY;
    }
    struct termios raw = ui->termios;
    raw.c_iflag &= (tcflag_t)~(BRKINT | ICRNL | INPCK | ISTRIP | IXON);
    raw.c_oflag &= (tcflag_t)~OPOST;
    raw.c_cflag |= CS8;
    raw.c_lflag &= (tcflag_t)~(ECHO | ICANON | IEXTEN | ISIG);
    raw.c_cc[VMIN] = 0;
    raw.c_cc[VTIME] = 0;
    if (tcsetattr(STDIN_FILENO, TCSAFLUSH, &raw) != 0) {
        int error = errno;
        free(ui);
        return error;
    }
#endif
    fputs("\x1b[?1049h\x1b[?25l", stdout);
    fflush(stdout);
    ui->active = 1;
    *out = ui;
    return 0;
}

void cclover_tui_leave(CcloverTui *ui) {
    if (ui == NULL) return;
    if (ui->active) {
        fputs("\x1b[?25h\x1b[?1049l", stdout);
        fflush(stdout);
#ifdef _WIN32
        SetConsoleMode(ui->input, ui->input_mode);
        SetConsoleMode(ui->output, ui->output_mode);
#else
        tcsetattr(STDIN_FILENO, TCSAFLUSH, &ui->termios);
#endif
    }
    free(ui);
}

int cclover_tui_draw(CcloverTui *ui, const CcloverTuiFrame *frame) {
    if (ui == NULL || frame == NULL) return EINVAL;
    unsigned width, height;
    terminal_size(&width, &height);
    fputs("\x1b[2J", stdout);
    CcloverText heading = {(const uint8_t *)"cclover-mon  ·  q / Esc / Ctrl-C: quit", 39};
    write_text(0, 0, width, heading);

    if (height <= 1) { fflush(stdout); return 0; }
    unsigned y = 1;
    unsigned overview_h = height - y < 9 ? height - y : 9;
    unsigned left_w = width / 2;
    if (left_w < 2) left_w = width;
    unsigned right_w = width - left_w;
    draw_panel(&frame->cpu, 0, y, left_w, overview_h, 1);
    if (right_w >= 2) draw_panel(&frame->memory, left_w, y, right_w, overview_h, 1);
    y += overview_h;

    unsigned gpu_h = y < height ? (height - y < 5 ? height - y : 5) : 0;
    if (gpu_h >= 2) draw_panel(&frame->gpu, 0, y, width, gpu_h, 0);
    y += gpu_h;
    unsigned temp_h = y < height ? (height - y < 5 ? height - y : 5) : 0;
    if (temp_h >= 2) draw_panel(&frame->temperatures, 0, y, width, temp_h, 0);
    y += temp_h;

    unsigned fan_h = frame->fans.row_count != 0 && y < height ? (height - y < 5 ? height - y : 5) : 0;
    if (fan_h >= 2) draw_panel(&frame->fans, 0, y, width, fan_h, 0);
    y += fan_h;

    if (y < height) {
        unsigned io_h = height - y;
        draw_panel(&frame->disks, 0, y, left_w, io_h, 0);
        if (right_w >= 2) draw_panel(&frame->networks, left_w, y, right_w, io_h, 0);
    }
    fflush(stdout);
    return ferror(stdout) ? EIO : 0;
}

int cclover_tui_wait_for_quit(CcloverTui *ui, uint32_t timeout_ms, int *quit) {
    if (ui == NULL || quit == NULL) return EINVAL;
    *quit = 0;
#ifdef _WIN32
    DWORD wait = WaitForSingleObject(ui->input, timeout_ms);
    if (wait == WAIT_TIMEOUT) return 0;
    if (wait != WAIT_OBJECT_0) return EIO;
    INPUT_RECORD records[32];
    DWORD count = 0;
    if (!ReadConsoleInputW(ui->input, records, 32, &count)) return EIO;
    for (DWORD i = 0; i < count; ++i) {
        if (records[i].EventType != KEY_EVENT || !records[i].Event.KeyEvent.bKeyDown) continue;
        KEY_EVENT_RECORD key = records[i].Event.KeyEvent;
        wchar_t ch = key.uChar.UnicodeChar;
        if (key.wVirtualKeyCode == VK_ESCAPE || ch == L'q' || ch == L'Q' ||
            ((key.dwControlKeyState & (LEFT_CTRL_PRESSED | RIGHT_CTRL_PRESSED)) && (ch == L'c' || ch == L'C'))) {
            *quit = 1;
            return 0;
        }
    }
#else
    struct pollfd fd = {.fd = STDIN_FILENO, .events = POLLIN, .revents = 0};
    int result;
    do {
        result = poll(&fd, 1, (int)timeout_ms);
    } while (result < 0 && errno == EINTR);
    if (result < 0) return errno;
    if (result == 0) return 0;
    uint8_t buffer[64];
    ssize_t count = read(STDIN_FILENO, buffer, sizeof(buffer));
    if (count < 0 && errno != EAGAIN && errno != EWOULDBLOCK) return errno;
    for (ssize_t i = 0; i < count; ++i) {
        if (buffer[i] == 0x1b || buffer[i] == 'q' || buffer[i] == 'Q' || buffer[i] == 0x03) {
            *quit = 1;
            break;
        }
    }
#endif
    return 0;
}
