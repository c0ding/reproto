use super::block::Block;
use super::statement::Statement;
use super::modifier::Modifiers;

#[derive(Debug, Clone)]
pub enum Section {
    Block(Block),
    Statement(Statement),
    Literal(Vec<String>),
    Spacing,
}

pub trait AsSection {
    fn as_section(self) -> Section;
}

impl<'a, A> AsSection for &'a A
    where A: AsSection + Clone
{
    fn as_section(self) -> Section {
        self.clone().as_section()
    }
}

impl<'a> AsSection for &'a str {
    fn as_section(self) -> Section {
        Section::Literal(vec![self.to_owned()])
    }
}

impl AsSection for String {
    fn as_section(self) -> Section {
        Section::Literal(vec![self.clone()])
    }
}

impl AsSection for Section {
    fn as_section(self) -> Section {
        self
    }
}

impl AsSection for Block {
    fn as_section(self) -> Section {
        Section::Block(self)
    }
}

impl AsSection for Statement {
    fn as_section(self) -> Section {
        Section::Statement(self)
    }
}

impl AsSection for Vec<String> {
    fn as_section(self) -> Section {
        Section::Literal(self)
    }
}

impl AsSection for Modifiers {
    fn as_section(self) -> Section {
        Section::Literal(vec![self.format()])
    }
}

#[derive(Debug, Clone)]
pub struct Sections {
    pub sections: Vec<Section>,
}

impl Sections {
    pub fn new() -> Sections {
        Sections { sections: Vec::new() }
    }

    pub fn push<S>(&mut self, section: S)
        where S: AsSection
    {
        self.sections.push(section.as_section());
    }

    pub fn extend(&mut self, sections: &Sections) {
        self.sections.extend(sections.sections.iter().map(Clone::clone));
    }

    pub fn format(&self, level: usize, current: &str, indent: &str) -> Vec<String> {
        let mut out = Vec::new();

        for section in &self.sections {
            match *section {
                Section::Statement(ref statement) => {
                    for line in statement.format(level) {
                        out.push(format!("{}{};", current, line));
                    }
                }
                Section::Block(ref block) => {
                    out.extend(block.format(level, current, indent));
                }
                Section::Spacing => {
                    out.push("".to_owned());
                }
                Section::Literal(ref content) => {
                    for line in content {
                        out.push(format!("{}{}", current, line));
                    }
                }
            }
        }

        out
    }
}
