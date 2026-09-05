//! Crossterm event normalization into protocol-neutral editor input.

use std::time::Duration;

use crossterm::event::{
    Event, KeyCode as CrosstermKeyCode, KeyEvent as CrosstermKeyEvent, KeyEventKind, KeyModifiers,
    MouseButton as CrosstermMouseButton, MouseEvent as CrosstermMouseEvent, MouseEventKind,
};
use editor_types::{
    InputEvent, KeyCode, KeyEvent, Modifier, Modifiers, MouseAction, MouseButton, MouseEvent,
    ScreenCell,
};

const SCROLL_LINES: i16 = 3;

/// Event-driven terminal input implemented separately from frame presentation.
pub trait InputReader {
    type Error: std::error::Error + Send + Sync + 'static;

    /// Blocks until the next supported normalized event is available.
    ///
    /// # Errors
    ///
    /// Returns an adapter error when terminal input fails.
    fn read_input(&mut self) -> Result<InputEvent, Self::Error>;

    /// Reports whether terminal input is ready without busy polling.
    ///
    /// # Errors
    ///
    /// Returns an adapter error when the host input handle cannot be polled.
    fn poll_input(&mut self, timeout: Duration) -> Result<bool, Self::Error>;
}

/// Normalizes one crossterm event. Focus and unsupported legacy events are intentionally ignored.
#[must_use]
pub fn normalize_event(event: Event) -> Option<InputEvent> {
    match event {
        Event::Key(key) => normalize_key(key).map(InputEvent::Key),
        Event::Mouse(mouse) => normalize_mouse(mouse).map(InputEvent::Mouse),
        Event::Paste(text) => Some(InputEvent::Paste(text)),
        Event::Resize(columns, rows) => Some(InputEvent::Resize { columns, rows }),
        Event::FocusGained | Event::FocusLost => None,
    }
}

/// Normalizes key presses and repeats while filtering release reports.
#[must_use]
pub fn normalize_key(event: CrosstermKeyEvent) -> Option<KeyEvent> {
    if event.kind == KeyEventKind::Release {
        return None;
    }
    let mut modifiers = event.modifiers;
    let code = match event.code {
        CrosstermKeyCode::Backspace => KeyCode::Backspace,
        CrosstermKeyCode::Enter => KeyCode::Enter,
        CrosstermKeyCode::Left => KeyCode::Left,
        CrosstermKeyCode::Right => KeyCode::Right,
        CrosstermKeyCode::Up => KeyCode::Up,
        CrosstermKeyCode::Down => KeyCode::Down,
        CrosstermKeyCode::Home => KeyCode::Home,
        CrosstermKeyCode::End => KeyCode::End,
        CrosstermKeyCode::PageUp => KeyCode::PageUp,
        CrosstermKeyCode::PageDown => KeyCode::PageDown,
        CrosstermKeyCode::Tab => KeyCode::Tab,
        CrosstermKeyCode::BackTab => {
            modifiers.insert(KeyModifiers::SHIFT);
            KeyCode::Tab
        }
        CrosstermKeyCode::Delete => KeyCode::Delete,
        CrosstermKeyCode::F(number) => KeyCode::Function(number),
        CrosstermKeyCode::Char(character) => KeyCode::Character(character),
        CrosstermKeyCode::Esc => KeyCode::Escape,
        CrosstermKeyCode::Insert
        | CrosstermKeyCode::Null
        | CrosstermKeyCode::CapsLock
        | CrosstermKeyCode::ScrollLock
        | CrosstermKeyCode::NumLock
        | CrosstermKeyCode::PrintScreen
        | CrosstermKeyCode::Pause
        | CrosstermKeyCode::Menu
        | CrosstermKeyCode::KeypadBegin
        | CrosstermKeyCode::Media(_)
        | CrosstermKeyCode::Modifier(_) => return None,
    };
    Some(KeyEvent {
        code,
        modifiers: normalize_modifiers(modifiers),
        repeat: event.kind == KeyEventKind::Repeat,
    })
}

/// Normalizes buttons, dragging, movement, and vertical wheel motion.
#[must_use]
pub fn normalize_mouse(event: CrosstermMouseEvent) -> Option<MouseEvent> {
    let action = match event.kind {
        MouseEventKind::Down(button) => MouseAction::Down(normalize_button(button)),
        MouseEventKind::Up(button) => MouseAction::Up(normalize_button(button)),
        MouseEventKind::Drag(button) => MouseAction::Drag(normalize_button(button)),
        MouseEventKind::Moved => MouseAction::Move,
        MouseEventKind::ScrollUp => MouseAction::ScrollLines(-SCROLL_LINES),
        MouseEventKind::ScrollDown => MouseAction::ScrollLines(SCROLL_LINES),
        MouseEventKind::ScrollLeft | MouseEventKind::ScrollRight => return None,
    };
    Some(MouseEvent {
        position: ScreenCell {
            row: event.row,
            column: event.column,
        },
        action,
        modifiers: normalize_modifiers(event.modifiers),
    })
}

fn normalize_button(button: CrosstermMouseButton) -> MouseButton {
    match button {
        CrosstermMouseButton::Left => MouseButton::Left,
        CrosstermMouseButton::Right => MouseButton::Right,
        CrosstermMouseButton::Middle => MouseButton::Middle,
    }
}

fn normalize_modifiers(modifiers: KeyModifiers) -> Modifiers {
    let mut normalized = Vec::with_capacity(4);
    if modifiers.contains(KeyModifiers::CONTROL) {
        normalized.push(Modifier::Control);
    }
    if modifiers.contains(KeyModifiers::ALT) {
        normalized.push(Modifier::Alt);
    }
    if modifiers.contains(KeyModifiers::SHIFT) {
        normalized.push(Modifier::Shift);
    }
    if modifiers.intersects(KeyModifiers::META | KeyModifiers::SUPER | KeyModifiers::HYPER) {
        normalized.push(Modifier::Meta);
    }
    Modifiers::from_modifiers(normalized)
}

#[cfg(test)]
mod tests {
    use crossterm::event::{
        Event, KeyCode as CrosstermKeyCode, KeyEvent as CrosstermKeyEvent, KeyEventKind,
        KeyEventState, KeyModifiers, MouseEvent as CrosstermMouseEvent, MouseEventKind,
    };
    use editor_types::{InputEvent, KeyCode, Modifier, MouseAction};

    use super::{normalize_event, normalize_key, normalize_mouse};

    #[test]
    fn key_normalization_preserves_modifiers_and_repeat() {
        let key = CrosstermKeyEvent {
            code: CrosstermKeyCode::Char('s'),
            modifiers: KeyModifiers::CONTROL | KeyModifiers::SHIFT,
            kind: KeyEventKind::Repeat,
            state: KeyEventState::NONE,
        };
        let normalized = normalize_key(key).expect("key is supported");
        assert_eq!(normalized.code, KeyCode::Character('s'));
        assert!(normalized.modifiers.contains(Modifier::Control));
        assert!(normalized.modifiers.contains(Modifier::Shift));
        assert!(normalized.repeat);
    }

    #[test]
    fn key_release_is_not_duplicated_as_an_action() {
        let key = CrosstermKeyEvent::new_with_kind(
            CrosstermKeyCode::Enter,
            KeyModifiers::NONE,
            KeyEventKind::Release,
        );
        assert_eq!(normalize_key(key), None);
    }

    #[test]
    fn mouse_normalization_preserves_coordinates_and_scroll_direction() {
        let mouse = CrosstermMouseEvent {
            kind: MouseEventKind::ScrollUp,
            column: 7,
            row: 4,
            modifiers: KeyModifiers::ALT,
        };
        let normalized = normalize_mouse(mouse).expect("vertical scroll is supported");
        assert_eq!(normalized.position.column, 7);
        assert_eq!(normalized.position.row, 4);
        assert_eq!(normalized.action, MouseAction::ScrollLines(-3));
        assert!(normalized.modifiers.contains(Modifier::Alt));
    }

    #[test]
    fn resize_is_normalized() {
        assert_eq!(
            normalize_event(Event::Resize(120, 40)),
            Some(InputEvent::Resize {
                columns: 120,
                rows: 40
            })
        );
    }
}
