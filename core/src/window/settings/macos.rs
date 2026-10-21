//! Platform specific settings for macOS.

/// The platform specific window settings of an application.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct PlatformSpecific {
    /// Hides the window title.
    pub title_hidden: bool,
    /// Makes the titlebar transparent and allows the content to appear behind it.
    pub titlebar_transparent: bool,
    /// Makes the window content appear behind the titlebar.
    pub fullsize_content_view: bool,
    /// Opens the window as a popup of another window, the way a Wayland popup surface is a
    /// child of its parent: a borderless, non-activating panel at the pop-up menu level,
    /// with a shadow. A [`Position::Specific`] position is then relative to the parent's
    /// content area, and the popup is moved to stay on the parent's monitor.
    ///
    /// The panel never becomes the key window, so the parent keeps keyboard focus and the
    /// popup still gets hover events.
    ///
    /// [`Position::Specific`]: crate::window::Position::Specific
    pub popup_of: Option<crate::window::Id>,
    /// For a popup: the x position, relative to the parent's content area, to use instead
    /// when the popup would cross the right edge of the screen. A submenu passes the left
    /// side of the item that opened it, so it flips to that side as a native menu does.
    /// `None` shifts the popup left just far enough.
    pub popup_flip_x: Option<f32>,
}
