#include <errno.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>

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
    unsigned width;
    unsigned height;
    int active;
} CcloverTui;

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
    terminal_size(&ui->width, &ui->height);
    *out = ui;
    return 0;
}

void cclover_tui_leave(CcloverTui *ui) {
    if (ui == NULL) return;
    if (ui->active) {
        fputs("\x1b[0m\x1b[?25h\x1b[?1049l", stdout);
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

int cclover_tui_size(CcloverTui *ui, uint32_t *width, uint32_t *height) {
    if (ui == NULL || width == NULL || height == NULL) return EINVAL;
    unsigned current_width, current_height;
    terminal_size(&current_width, &current_height);
    *width = current_width;
    *height = current_height;
    return 0;
}

int cclover_tui_draw(CcloverTui *ui, const CcloverTuiFrame *frame) {
    if (ui == NULL || frame == NULL) return EINVAL;
    unsigned width, height;
    terminal_size(&width, &height);
    ui->width = width;
    ui->height = height;

    fputs("\x1b[H\x1b[2J", stdout);
    size_t count = frame->line_count < height ? frame->line_count : height;
    for (size_t row = 0; row < count; ++row) {
        const CcloverText line = frame->lines[row];
        if (line.ptr != NULL && line.len != 0) fwrite(line.ptr, 1, line.len, stdout);
        fputs("\x1b[0m", stdout);
        if (row + 1 < count) fputs("\r\n", stdout);
    }
    fflush(stdout);
    return ferror(stdout) ? EIO : 0;
}

int cclover_tui_size_changed(CcloverTui *ui, int *changed) {
    if (ui == NULL || changed == NULL) return EINVAL;
    unsigned width, height;
    terminal_size(&width, &height);
    *changed = width != ui->width || height != ui->height;
    return 0;
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
