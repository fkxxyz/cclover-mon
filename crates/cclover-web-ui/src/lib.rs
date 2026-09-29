mod graph;

use cclover_presentation::Dashboard;
use cclover_ui::{
    Block, Card, DashboardUi, Element as UiElement, GraphSpec, Stack, TextCell, TextRow,
    TextWeight, Tone,
};
use graph::Graph;
use iced::border;
use iced::font::Weight;
use iced::widget::text::Wrapping;
use iced::widget::{Column, Space, canvas, column, container, progress_bar, row, text};
use iced::{Alignment, Border, Color, Element, Fill, Font, Theme};

pub use cclover_ui::{INITIAL_PANEL_HEIGHT, PANEL_WIDTH};

pub struct PanelState {
    surface_height: u32,
}

impl PanelState {
    pub fn new(_dashboard: Dashboard<'_>) -> Self {
        Self {
            surface_height: INITIAL_PANEL_HEIGHT,
        }
    }

    pub fn update(&mut self, dashboard: Dashboard<'_>) -> Option<u32> {
        let next_height = DashboardUi::new(dashboard).height();
        if next_height == self.surface_height {
            return None;
        }
        self.surface_height = next_height;
        Some(next_height)
    }

    pub fn surface_height(&self) -> u32 {
        self.surface_height
    }

    pub fn view<'a, Message: 'a + 'static>(
        &'a self,
        dashboard: Dashboard<'a>,
    ) -> Element<'a, Message> {
        view(dashboard)
    }
}

#[cfg(all(not(target_os = "windows"), not(target_arch = "wasm32")))]
const MONO: Font = Font::with_name("Inconsolata");
#[cfg(any(target_os = "windows", target_arch = "wasm32"))]
const MONO: Font = Font::with_name("Fira Sans");
const MONO_BOLD: Font = Font {
    weight: Weight::Bold,
    ..MONO
};

pub fn view<'a, Message>(dashboard: Dashboard<'a>) -> Element<'a, Message>
where
    Message: 'a + 'static,
{
    let ui = DashboardUi::new(dashboard);
    let left = panel_column(ui.left);
    let right = panel_column(ui.right);
    let body = row![left, right]
        .spacing(cclover_ui::PANEL_GEOMETRY.column_spacing)
        .width(Fill)
        .align_y(Alignment::Start);

    container(body)
        .padding(cclover_ui::PANEL_GEOMETRY.padding as u16)
        .width(Fill)
        .style(|_| container::Style {
            background: Some(color(Tone::Background).into()),
            border: Border {
                color: color(Tone::Border),
                width: 1.0,
                radius: 16.0.into(),
            },
            ..container::Style::default()
        })
        .into()
}

fn panel_column<'a, Message>(blocks: Vec<Block<'a>>) -> Column<'a, Message>
where
    Message: 'a + 'static,
{
    let mut result = column![]
        .spacing(cclover_ui::PANEL_GEOMETRY.column_spacing)
        .width(Fill);
    for block in blocks {
        result = result.push(block_view(block));
    }
    result
}

fn block_view<'a, Message>(block: Block<'a>) -> Element<'a, Message>
where
    Message: 'a + 'static,
{
    match block {
        Block::Section(label) => container(text_cell(label))
            .height(cclover_ui::PANEL_GEOMETRY.section_height)
            .width(Fill)
            .into(),
        Block::Card(card) => card_view(card),
    }
}

fn card_view<'a, Message>(card: Card<'a>) -> Element<'a, Message>
where
    Message: 'a + 'static,
{
    let height = card.height();
    container(stack_view(card.content))
        .padding(card.padding as u16)
        .height(height)
        .width(Fill)
        .style(|_| container::Style {
            background: Some(color(Tone::Card).into()),
            border: Border {
                color: color(Tone::Border),
                width: 1.0,
                radius: 12.0.into(),
            },
            ..container::Style::default()
        })
        .into()
}

fn stack_view<'a, Message>(stack: Stack<'a>) -> Column<'a, Message>
where
    Message: 'a + 'static,
{
    let mut result = column![].spacing(stack.gap).width(Fill);
    if let Some(height) = stack.height {
        result = result.height(height);
    }
    for child in stack.children {
        result = result.push(element_view(child));
    }
    result
}

fn element_view<'a, Message>(element: UiElement<'a>) -> Element<'a, Message>
where
    Message: 'a + 'static,
{
    match element {
        UiElement::Row(row) => row_view(row),
        UiElement::Graph(graph) => graph_view(graph),
        UiElement::Progress(progress) => progress_bar(0.0..=1.0, progress.value)
            .girth(progress.height)
            .style(move |_| iced::widget::progress_bar::Style {
                background: color(Tone::Border).into(),
                bar: color(progress.tone).into(),
                border: border::rounded(3),
            })
            .into(),
        UiElement::Stack(stack) => stack_view(stack).into(),
    }
}

fn row_view<'a, Message>(row_spec: TextRow<'a>) -> Element<'a, Message>
where
    Message: 'a + 'static,
{
    let mut result = row![]
        .spacing(row_spec.gap)
        .height(row_spec.height)
        .align_y(Alignment::Center);
    for cell in row_spec.cells {
        if cell.text.is_empty() && cell.grow {
            result = result.push(Space::new().width(Fill));
            continue;
        }
        let grow = cell.grow;
        let clip = cell.clip;
        let mut label = text_cell(cell);
        if grow {
            label = label.width(Fill);
        }
        if clip {
            result = result.push(
                container(label.wrapping(Wrapping::None))
                    .width(if grow { Fill } else { iced::Length::Shrink })
                    .clip(true),
            );
        } else {
            result = result.push(label);
        }
    }
    result.into()
}

fn graph_view<'a, Message>(spec: GraphSpec<'a>) -> Element<'a, Message>
where
    Message: 'a + 'static,
{
    let mut area = spec.line.rgba();
    area.a = spec.fill_alpha;
    let graph = Graph::new(spec.values, spec.capacity, color(spec.line), rgba(area))
        .range(spec.min, spec.max)
        .auto_scale(spec.auto_scale);
    canvas(graph).width(Fill).height(spec.height).into()
}

fn text_cell(cell: TextCell<'_>) -> iced::widget::Text<'static> {
    let font = match cell.weight {
        TextWeight::Regular => MONO,
        TextWeight::Bold => MONO_BOLD,
    };
    text(cell.text.into_owned())
        .font(font)
        .size(cell.size)
        .color(color(cell.tone))
}

fn color(tone: Tone) -> Color {
    rgba(tone.rgba())
}

fn rgba(value: cclover_ui::Rgba) -> Color {
    Color::from_rgba8(value.r, value.g, value.b, value.a)
}

pub fn theme() -> Theme {
    Theme::Dark
}
