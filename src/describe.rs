/// A general description with an additional title field.
pub struct Description {
    title: &'static str,
    desc: String,
}

impl Description {
    /// Generates a new [`Description`] from a **title** and the actual **description**.
    pub fn new(title: &'static str, desc: String) -> Self {
        Self { title, desc }
    }

    /// Returns the **title** of `self`.
    pub fn title(&self) -> &'static str {
        self.title
    }

    /// Returns the **description** of `self`.
    pub fn desc(&self) -> &str {
        self.desc.as_str()
    }
}

pub trait Describe {
    /// Describes a type by providing a [`Description`] for it.
    fn describe(&self) -> Description;
}
