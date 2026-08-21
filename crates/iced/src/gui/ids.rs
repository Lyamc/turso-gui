use iced::widget::Id;

#[derive(Debug, Clone)]
pub struct ScrollIds {
    pub header: Id,
    pub row_index: Id,
    pub data: Id,
}

impl Default for ScrollIds {
    fn default() -> Self {
        Self {
            header: Id::unique(),
            row_index: Id::unique(),
            data: Id::unique(),
        }
    }
}
