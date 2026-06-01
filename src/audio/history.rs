use crate::audio::buffer::AudioBuffer;
use crate::error::Result;

#[derive(Clone, Debug)]
struct HistoryEntry {
    label: String,
    buffer: AudioBuffer,
}

#[derive(Clone, Debug)]
pub struct AudioHistory {
    undo_stack: Vec<HistoryEntry>,
    redo_stack: Vec<HistoryEntry>,
    max_entries: usize,
}

#[derive(Clone, Debug)]
pub struct AudioDocument {
    buffer: AudioBuffer,
    history: AudioHistory,
}

impl Default for AudioHistory {
    fn default() -> Self {
        Self {
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            max_entries: 32,
        }
    }
}

impl AudioHistory {
    pub fn can_undo(&self) -> bool {
        !self.undo_stack.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo_stack.is_empty()
    }

    fn push_undo(&mut self, label: String, buffer: AudioBuffer) {
        self.undo_stack.push(HistoryEntry { label, buffer });
        if self.undo_stack.len() > self.max_entries {
            self.undo_stack.remove(0);
        }
        self.redo_stack.clear();
    }
}

impl AudioDocument {
    pub fn new(buffer: AudioBuffer) -> Self {
        Self {
            buffer,
            history: AudioHistory::default(),
        }
    }

    pub fn replace_buffer(&mut self, buffer: AudioBuffer) {
        self.buffer = buffer;
        self.history = AudioHistory::default();
    }

    pub fn overwrite_buffer(&mut self, buffer: AudioBuffer) {
        self.buffer = buffer;
    }

    pub fn buffer(&self) -> &AudioBuffer {
        &self.buffer
    }

    pub fn can_undo(&self) -> bool {
        self.history.can_undo()
    }

    pub fn can_redo(&self) -> bool {
        self.history.can_redo()
    }

    pub fn apply_edit<F>(&mut self, label: impl Into<String>, edit: F) -> Result<()>
    where
        F: FnOnce(&mut AudioBuffer) -> Result<()>,
    {
        let label = label.into();
        let before = self.buffer.clone();
        let mut next = self.buffer.clone();
        edit(&mut next)?;

        self.history.push_undo(label, before);
        self.buffer = next;
        Ok(())
    }

    pub fn undo(&mut self) -> Result<Option<String>> {
        let Some(entry) = self.history.undo_stack.pop() else {
            return Ok(None);
        };

        let current = self.buffer.clone();
        self.history.redo_stack.push(HistoryEntry {
            label: entry.label.clone(),
            buffer: current,
        });
        self.buffer = entry.buffer;

        Ok(Some(entry.label))
    }

    pub fn redo(&mut self) -> Result<Option<String>> {
        let Some(entry) = self.history.redo_stack.pop() else {
            return Ok(None);
        };

        let current = self.buffer.clone();
        self.history.undo_stack.push(HistoryEntry {
            label: entry.label.clone(),
            buffer: current,
        });
        self.buffer = entry.buffer;

        Ok(Some(entry.label))
    }
}
