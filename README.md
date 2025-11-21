# DevMark - Markdown Editor for Developers

A lightweight, code-first Markdown editor built with Flutter, specifically designed for software developers.

## Overview

DevMark is a cross-platform (macOS, Windows, Linux) Markdown editor that focuses on what developers need most:
- **Code-first editing** with syntax highlighting for multiple programming languages
- **Project/Repository structure** awareness with filtered file tree
- **Developer templates** for common documentation (README, CHANGELOG, ADR, API docs, etc.)
- **Fast and clean UI** without bloat

## Features

### ✨ MVP Features (Implemented)

#### Editor
- Full Markdown support (headings, lists, links, images, code blocks, quotes, task lists)
- Undo/Redo functionality
- Rich toolbar with quick-insert buttons for Markdown elements
- Real-time editing with live preview

#### Code Blocks
- Syntax highlighting for Dart, TypeScript, JavaScript, C#, Java, Python, Go, Rust, and more
- Copy button for each code block
- Customizable code themes (Light/Dark mode)
- Line numbers support

#### Project View
- Sidebar with file tree filtered to show only Markdown files (.md, .markdown, .mdx)
- Multi-tab support for working with multiple documents
- Auto-save with dirty state indication
- Git-aware workspace management

#### Preview
- Live Markdown preview with custom code renderer
- Three view modes: Editor only, Split view, Preview only
- Seamless view mode switching

#### Templates
- Built-in templates for common developer documentation:
  - README.md
  - CHANGELOG.md
  - ADR (Architecture Decision Records)
  - API Documentation
  - CONTRIBUTING.md
  - CODE_OF_CONDUCT.md
  - LICENSE (MIT)
- Quick document creation from templates

## Architecture

### Core Structure

```
lib/
├── models/
│   ├── document.dart       # Document model with content and dirty state
│   └── workspace.dart      # Workspace managing folders and documents
├── services/
│   ├── file_service.dart   # File system operations
│   └── template_service.dart # Template management
├── widgets/
│   ├── markdown_editor.dart   # Editor widget with toolbar
│   ├── markdown_preview.dart  # Preview widget with code highlighting
│   └── file_tree.dart         # Sidebar file tree
├── screens/
│   └── main_shell.dart     # Main app shell with layout
└── main.dart               # App entry point
```

### Key Components

- **Document**: Manages content, file path, and dirty state with ChangeNotifier
- **Workspace**: Manages multiple documents and active workspace folder
- **FileService**: Handles reading, writing, and listing Markdown files
- **TemplateService**: Provides developer-focused document templates
- **MarkdownEditor**: Code editor with Markdown syntax highlighting and toolbar
- **MarkdownPreview**: Live preview with enhanced code block rendering
- **MainShell**: Main UI with sidebar, tabs, editor/preview area, and status bar

## Dependencies

- `flutter_markdown` - Markdown rendering
- `flutter_code_editor` - Code editing with syntax highlighting
- `flutter_highlight` - Syntax highlighting themes
- `file_selector` - Native file/folder picker
- `provider` - State management
- `path` - Path manipulation utilities

## Installation

### Prerequisites

- Flutter SDK (3.0.0 or higher)
- Dart SDK (included with Flutter)

### Setup

```bash
# Clone the repository
git clone https://github.com/yourusername/devmark.git
cd devmark

# Install dependencies
flutter pub get

# Run on your platform
flutter run -d macos    # macOS
flutter run -d windows  # Windows
flutter run -d linux    # Linux
```

## Usage

### Opening a Workspace
1. Click the menu icon (☰) to open the sidebar
2. Click "Open Folder" to select a directory containing Markdown files
3. All .md, .markdown, and .mdx files will be displayed in the file tree

### Creating Documents
1. Open the sidebar
2. Click "New from Template"
3. Select a template (README, CHANGELOG, ADR, etc.)
4. Edit and save

### Editing Documents
- Use the toolbar for quick Markdown formatting
- Switch between Editor, Split, and Preview modes using the view button
- Toggle between light and dark themes using the theme button
- Save with the save button or Cmd/Ctrl+S

### Working with Tabs
- Multiple documents can be opened in tabs
- Close tabs with the × button
- Dirty (unsaved) documents show a dot indicator

## Keyboard Shortcuts

- **Save**: Cmd+S (macOS) / Ctrl+S (Windows/Linux)
- **Undo**: Cmd+Z / Ctrl+Z
- **Redo**: Cmd+Shift+Z / Ctrl+Shift+Z

## Future Enhancements (Phase 2)

- [ ] Outline view for heading navigation
- [ ] Markdown linting (heading rules, empty headings, missing code block languages)
- [ ] Frontmatter support (YAML/TOML)
- [ ] Export to HTML/PDF with developer themes
- [ ] Custom snippets and shortcodes
- [ ] GitHub/Gist integration
- [ ] Find and replace with regex
- [ ] Multi-cursor editing

## Development

### Project Structure

The codebase follows clean architecture principles:
- **Models**: Data structures with business logic
- **Services**: Business operations and I/O
- **Widgets**: Reusable UI components
- **Screens**: Complete app screens/pages

### Contributing

Contributions are welcome! Please feel free to submit a Pull Request.

## License

This project is licensed under the MIT License - see the LICENSE file for details.

## Acknowledgments

- Built with [Flutter](https://flutter.dev/)
- Inspired by the need for a clean, developer-focused Markdown editor
- Based on the concept document in `todo.txt`
