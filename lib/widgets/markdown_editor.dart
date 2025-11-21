import 'package:flutter/material.dart';
import 'package:flutter_code_editor/flutter_code_editor.dart';
import 'package:flutter_highlight/themes/github.dart';
import 'package:flutter_highlight/themes/monokai-sublime.dart';
import 'package:highlight/languages/markdown.dart';
import '../models/document.dart';

/// Markdown editor widget with syntax highlighting and toolbar
class MarkdownEditor extends StatefulWidget {
  final Document? document;
  final ValueChanged<String>? onChanged;
  final bool isDarkMode;

  const MarkdownEditor({
    super.key,
    this.document,
    this.onChanged,
    this.isDarkMode = false,
  });

  @override
  State<MarkdownEditor> createState() => _MarkdownEditorState();
}

class _MarkdownEditorState extends State<MarkdownEditor> {
  late CodeController _controller;

  @override
  void initState() {
    super.initState();
    _initializeController();
  }

  @override
  void didUpdateWidget(MarkdownEditor oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (oldWidget.document != widget.document) {
      _initializeController();
    }
  }

  void _initializeController() {
    _controller = CodeController(
      text: widget.document?.content ?? '',
      language: markdown,
    );

    _controller.addListener(() {
      if (widget.document != null) {
        widget.document!.content = _controller.text;
      }
      widget.onChanged?.call(_controller.text);
    });
  }

  @override
  void dispose() {
    _controller.dispose();
    super.dispose();
  }

  void _insertMarkdown(String prefix, {String suffix = '', String placeholder = ''}) {
    final selection = _controller.selection;
    final text = _controller.text;

    if (selection.isValid && !selection.isCollapsed) {
      // Wrap selected text
      final selectedText = text.substring(selection.start, selection.end);
      final newText = '$prefix$selectedText$suffix';
      _controller.text = text.replaceRange(selection.start, selection.end, newText);
      _controller.selection = TextSelection.collapsed(
        offset: selection.start + prefix.length + selectedText.length,
      );
    } else {
      // Insert at cursor with placeholder
      final offset = selection.baseOffset;
      final newText = '$prefix$placeholder$suffix';
      _controller.text = text.substring(0, offset) + newText + text.substring(offset);
      _controller.selection = TextSelection.collapsed(
        offset: offset + prefix.length + placeholder.length,
      );
    }
  }

  @override
  Widget build(BuildContext context) {
    return Column(
      children: [
        _buildToolbar(),
        Expanded(
          child: CodeTheme(
            data: CodeThemeData(
              styles: widget.isDarkMode ? monokaiSublimeTheme : githubTheme,
            ),
            child: SingleChildScrollView(
              child: CodeField(
                controller: _controller,
                textStyle: const TextStyle(
                  fontFamily: 'monospace',
                  fontSize: 14,
                ),
              ),
            ),
          ),
        ),
      ],
    );
  }

  Widget _buildToolbar() {
    return Container(
      padding: const EdgeInsets.symmetric(horizontal: 8, vertical: 4),
      decoration: BoxDecoration(
        color: Theme.of(context).colorScheme.surfaceVariant,
        border: Border(
          bottom: BorderSide(
            color: Theme.of(context).dividerColor,
          ),
        ),
      ),
      child: SingleChildScrollView(
        scrollDirection: Axis.horizontal,
        child: Row(
          children: [
            _toolbarButton(
              icon: Icons.format_bold,
              tooltip: 'Bold',
              onPressed: () => _insertMarkdown('**', suffix: '**', placeholder: 'bold text'),
            ),
            _toolbarButton(
              icon: Icons.format_italic,
              tooltip: 'Italic',
              onPressed: () => _insertMarkdown('*', suffix: '*', placeholder: 'italic text'),
            ),
            _toolbarButton(
              icon: Icons.strikethrough_s,
              tooltip: 'Strikethrough',
              onPressed: () => _insertMarkdown('~~', suffix: '~~', placeholder: 'strikethrough'),
            ),
            const VerticalDivider(),
            _toolbarButton(
              icon: Icons.title,
              tooltip: 'Heading',
              onPressed: () => _insertMarkdown('## ', placeholder: 'Heading'),
            ),
            _toolbarButton(
              icon: Icons.format_list_bulleted,
              tooltip: 'Bullet List',
              onPressed: () => _insertMarkdown('- ', placeholder: 'list item'),
            ),
            _toolbarButton(
              icon: Icons.format_list_numbered,
              tooltip: 'Numbered List',
              onPressed: () => _insertMarkdown('1. ', placeholder: 'list item'),
            ),
            _toolbarButton(
              icon: Icons.check_box,
              tooltip: 'Task List',
              onPressed: () => _insertMarkdown('- [ ] ', placeholder: 'task'),
            ),
            const VerticalDivider(),
            _toolbarButton(
              icon: Icons.code,
              tooltip: 'Inline Code',
              onPressed: () => _insertMarkdown('`', suffix: '`', placeholder: 'code'),
            ),
            _toolbarButton(
              icon: Icons.code_outlined,
              tooltip: 'Code Block',
              onPressed: () => _insertMarkdown('```\n', suffix: '\n```', placeholder: 'code'),
            ),
            _toolbarButton(
              icon: Icons.format_quote,
              tooltip: 'Quote',
              onPressed: () => _insertMarkdown('> ', placeholder: 'quote'),
            ),
            const VerticalDivider(),
            _toolbarButton(
              icon: Icons.link,
              tooltip: 'Link',
              onPressed: () => _insertMarkdown('[', suffix: '](url)', placeholder: 'link text'),
            ),
            _toolbarButton(
              icon: Icons.image,
              tooltip: 'Image',
              onPressed: () => _insertMarkdown('![', suffix: '](url)', placeholder: 'alt text'),
            ),
            _toolbarButton(
              icon: Icons.horizontal_rule,
              tooltip: 'Horizontal Rule',
              onPressed: () => _insertMarkdown('\n---\n'),
            ),
          ],
        ),
      ),
    );
  }

  Widget _toolbarButton({
    required IconData icon,
    required String tooltip,
    required VoidCallback onPressed,
  }) {
    return Padding(
      padding: const EdgeInsets.symmetric(horizontal: 2),
      child: IconButton(
        icon: Icon(icon, size: 20),
        tooltip: tooltip,
        onPressed: onPressed,
        padding: const EdgeInsets.all(8),
        constraints: const BoxConstraints(minWidth: 32, minHeight: 32),
      ),
    );
  }
}
