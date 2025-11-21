import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_markdown/flutter_markdown.dart';
import 'package:flutter_highlight/flutter_highlight.dart';
import 'package:flutter_highlight/themes/github.dart';
import 'package:flutter_highlight/themes/monokai-sublime.dart';
import 'package:markdown/markdown.dart' as md;
import '../models/document.dart';

/// Markdown preview widget with custom code block rendering
class MarkdownPreview extends StatelessWidget {
  final Document? document;
  final bool isDarkMode;

  const MarkdownPreview({
    super.key,
    this.document,
    this.isDarkMode = false,
  });

  @override
  Widget build(BuildContext context) {
    if (document == null) {
      return Center(
        child: Text(
          'No document selected',
          style: Theme.of(context).textTheme.bodyLarge,
        ),
      );
    }

    return Container(
      color: Theme.of(context).colorScheme.surface,
      child: Markdown(
        data: document!.content,
        selectable: true,
        extensionSet: md.ExtensionSet.gitHubFlavored,
        builders: {
          'code': CodeElementBuilder(isDarkMode: isDarkMode),
        },
        styleSheet: MarkdownStyleSheet.fromTheme(Theme.of(context)).copyWith(
          code: TextStyle(
            backgroundColor: isDarkMode ? Colors.grey[800] : Colors.grey[200],
            fontFamily: 'monospace',
            fontSize: 14,
          ),
          codeblockDecoration: BoxDecoration(
            color: isDarkMode ? Colors.grey[900] : Colors.grey[100],
            borderRadius: BorderRadius.circular(4),
          ),
          blockquotePadding: const EdgeInsets.all(16),
          blockquoteDecoration: BoxDecoration(
            color: isDarkMode ? Colors.grey[800] : Colors.grey[100],
            borderRadius: BorderRadius.circular(4),
            border: Border(
              left: BorderSide(
                color: Theme.of(context).colorScheme.primary,
                width: 4,
              ),
            ),
          ),
        ),
      ),
    );
  }
}

/// Custom builder for code blocks with syntax highlighting and copy button
class CodeElementBuilder extends MarkdownElementBuilder {
  final bool isDarkMode;

  CodeElementBuilder({required this.isDarkMode});

  @override
  Widget visitElementAfter(md.Element element, TextStyle? preferredStyle) {
    var language = '';

    if (element.attributes['class'] != null) {
      String classes = element.attributes['class'] as String;
      language = classes.substring(9); // Remove 'language-' prefix
    }

    final code = element.textContent;

    return CodeBlock(
      code: code,
      language: language,
      isDarkMode: isDarkMode,
    );
  }
}

/// Code block widget with syntax highlighting and copy button
class CodeBlock extends StatelessWidget {
  final String code;
  final String language;
  final bool isDarkMode;

  const CodeBlock({
    super.key,
    required this.code,
    required this.language,
    required this.isDarkMode,
  });

  @override
  Widget build(BuildContext context) {
    return Container(
      margin: const EdgeInsets.symmetric(vertical: 8),
      decoration: BoxDecoration(
        color: isDarkMode ? Colors.grey[900] : Colors.grey[100],
        borderRadius: BorderRadius.circular(8),
        border: Border.all(
          color: isDarkMode ? Colors.grey[700]! : Colors.grey[300]!,
        ),
      ),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          // Header with language and copy button
          Container(
            padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 8),
            decoration: BoxDecoration(
              color: isDarkMode ? Colors.grey[800] : Colors.grey[200],
              borderRadius: const BorderRadius.only(
                topLeft: Radius.circular(8),
                topRight: Radius.circular(8),
              ),
            ),
            child: Row(
              children: [
                if (language.isNotEmpty)
                  Text(
                    language,
                    style: TextStyle(
                      fontSize: 12,
                      fontWeight: FontWeight.w600,
                      color: isDarkMode ? Colors.grey[400] : Colors.grey[700],
                    ),
                  ),
                const Spacer(),
                IconButton(
                  icon: const Icon(Icons.content_copy, size: 16),
                  onPressed: () {
                    Clipboard.setData(ClipboardData(text: code));
                    ScaffoldMessenger.of(context).showSnackBar(
                      const SnackBar(
                        content: Text('Code copied to clipboard'),
                        duration: Duration(seconds: 2),
                      ),
                    );
                  },
                  tooltip: 'Copy code',
                  padding: const EdgeInsets.all(4),
                  constraints: const BoxConstraints(minWidth: 32, minHeight: 32),
                ),
              ],
            ),
          ),
          // Code content with syntax highlighting
          Container(
            padding: const EdgeInsets.all(16),
            child: HighlightView(
              code,
              language: language.isNotEmpty ? language : 'plaintext',
              theme: isDarkMode ? monokaiSublimeTheme : githubTheme,
              padding: EdgeInsets.zero,
              textStyle: const TextStyle(
                fontFamily: 'monospace',
                fontSize: 14,
              ),
            ),
          ),
        ],
      ),
    );
  }
}
