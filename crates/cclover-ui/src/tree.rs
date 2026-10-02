use std::borrow::Cow;
use std::collections::VecDeque;

use crate::{PANEL_GEOMETRY, TextAlign, TextWeight, Tone};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CellWidth {
    Fixed(u32),
    Fill,
}

#[derive(Debug, Clone)]
pub struct TextCell<'a> {
    pub text: Cow<'a, str>,
    pub size: u32,
    pub tone: Tone,
    pub weight: TextWeight,
    pub width: CellWidth,
    pub align: TextAlign,
    pub clip: bool,
    pub static_content: bool,
}

impl<'a> TextCell<'a> {
    pub(crate) fn borrowed(text: &'a str, size: u32, tone: Tone) -> Self {
        Self {
            text: Cow::Borrowed(text),
            size,
            tone,
            weight: TextWeight::Regular,
            width: CellWidth::Fill,
            align: TextAlign::Start,
            clip: false,
            static_content: false,
        }
    }

    pub(crate) fn owned(text: String, size: u32, tone: Tone) -> Self {
        Self {
            text: Cow::Owned(text),
            size,
            tone,
            weight: TextWeight::Regular,
            width: CellWidth::Fill,
            align: TextAlign::Start,
            clip: false,
            static_content: false,
        }
    }

    pub(crate) fn bold(mut self) -> Self {
        self.weight = TextWeight::Bold;
        self
    }

    pub(crate) fn fixed(mut self, width: u32) -> Self {
        self.width = CellWidth::Fixed(width);
        self.align = TextAlign::End;
        self.clip = true;
        self
    }

    pub(crate) fn fill(mut self) -> Self {
        self.width = CellWidth::Fill;
        self.align = TextAlign::Start;
        self
    }

    pub(crate) fn align_start(mut self) -> Self {
        self.align = TextAlign::Start;
        self
    }

    pub(crate) fn clip(mut self) -> Self {
        self.clip = true;
        self
    }

    pub(crate) fn static_content(mut self) -> Self {
        self.static_content = true;
        self
    }
}

#[derive(Debug, Clone)]
pub struct TextRow<'a> {
    pub cells: Vec<TextCell<'a>>,
    pub height: u32,
    pub gap: u32,
}

#[derive(Debug, Clone, Copy)]
pub struct GraphSpec<'a> {
    pub values: &'a VecDeque<f64>,
    pub min: f64,
    pub max: f64,
    pub auto_scale: bool,
    pub line: Tone,
    pub fill_alpha: f32,
    pub capacity: usize,
    pub height: u32,
}

impl GraphSpec<'_> {
    pub fn resolved_max(&self) -> f64 {
        if self.auto_scale {
            (self.values.iter().copied().fold(1024.0_f64, f64::max) * 1.12).max(self.min + 0.001)
        } else {
            self.max.max(self.min + 0.001)
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct ProgressSpec {
    pub value: f32,
    pub tone: Tone,
    pub height: u32,
}

#[derive(Debug, Clone)]
pub struct Stack<'a> {
    pub children: Vec<Element<'a>>,
    pub gap: u32,
    pub height: Option<u32>,
}

#[derive(Debug, Clone)]
pub enum Element<'a> {
    Row(TextRow<'a>),
    Graph(GraphSpec<'a>),
    Progress(ProgressSpec),
    Stack(Stack<'a>),
}

impl Element<'_> {
    pub fn height(&self) -> u32 {
        match self {
            Self::Row(row) => row.height,
            Self::Graph(graph) => graph.height,
            Self::Progress(progress) => progress.height,
            Self::Stack(stack) => stack.height.unwrap_or_else(|| stack.content_height()),
        }
    }
}

impl Stack<'_> {
    pub fn content_height(&self) -> u32 {
        let children = self.children.iter().map(Element::height).sum::<u32>();
        children + self.gap * self.children.len().saturating_sub(1) as u32
    }
}

#[derive(Debug, Clone)]
pub struct Card<'a> {
    pub content: Stack<'a>,
    pub padding: u32,
}

impl Card<'_> {
    pub fn height(&self) -> u32 {
        self.padding * 2
            + self
                .content
                .height
                .unwrap_or_else(|| self.content.content_height())
    }
}

#[derive(Debug, Clone)]
pub enum Block<'a> {
    Section(TextCell<'a>),
    Card(Card<'a>),
}

impl Block<'_> {
    pub fn height(&self) -> u32 {
        match self {
            Self::Section(_) => PANEL_GEOMETRY.section_height,
            Self::Card(card) => card.height(),
        }
    }
}
