import 'package:flutter/material.dart';
import 'package:provider/provider.dart';
import 'package:file_selector/file_selector.dart';
import '../models/workspace.dart';
import '../models/document.dart';
import '../services/file_service.dart';
import '../services/template_service.dart';
import '../widgets/markdown_editor.dart';
import '../widgets/markdown_preview.dart';
import '../widgets/file_tree.dart';

enum ViewMode { editor, split, preview }

/// Main application shell with sidebar, editor, and preview
class MainShell extends StatefulWidget {
  const MainShell({super.key});

  @override
  State<MainShell> createState() => _MainShellState();
}

class _MainShellState extends State<MainShell> {
  final FileService _fileService = FileService();
  final TemplateService _templateService = TemplateService();
  ViewMode _viewMode = ViewMode.split;
  bool _isDarkMode = false;
  List<String> _markdownFiles = [];
  bool _isLoading = false;

  @override
  void initState() {
    super.initState();
    _templateService.initialize();
  }

  Future<void> _openFolder() async {
    final directoryPath = await getDirectoryPath();

    if (directoryPath != null) {
      setState(() => _isLoading = true);

      try {
        final workspace = context.read<Workspace>();
        workspace.setFolderPath(directoryPath);

        final files = await _fileService.listMarkdownFiles(directoryPath);
        setState(() {
          _markdownFiles = files;
        });
      } catch (e) {
        if (mounted) {
          ScaffoldMessenger.of(context).showSnackBar(
            SnackBar(content: Text('Error opening folder: $e')),
          );
        }
      } finally {
        setState(() => _isLoading = false);
      }
    }
  }

  Future<void> _openFile(String filePath) async {
    try {
      final workspace = context.read<Workspace>();

      // Check if document is already open
      var document = workspace.getDocumentByPath(filePath);

      if (document == null) {
        // Load document from disk
        document = await _fileService.loadDocument(filePath);
        workspace.addDocument(document);
      }

      workspace.setActiveDocument(document);
    } catch (e) {
      if (mounted) {
        ScaffoldMessenger.of(context).showSnackBar(
          SnackBar(content: Text('Error opening file: $e')),
        );
      }
    }
  }

  Future<void> _saveDocument() async {
    final workspace = context.read<Workspace>();
    final document = workspace.activeDocument;

    if (document == null) return;

    try {
      await _fileService.saveDocument(document);

      if (mounted) {
        ScaffoldMessenger.of(context).showSnackBar(
          const SnackBar(content: Text('Document saved')),
        );
      }
    } catch (e) {
      if (mounted) {
        ScaffoldMessenger.of(context).showSnackBar(
          SnackBar(content: Text('Error saving document: $e')),
        );
      }
    }
  }

  Future<void> _newDocumentFromTemplate() async {
    final template = await showDialog<Template>(
      context: context,
      builder: (context) => _TemplateSelectionDialog(
        templates: _templateService.getAllTemplates(),
      ),
    );

    if (template == null) return;

    final workspace = context.read<Workspace>();
    final folderPath = workspace.folderPath;

    if (folderPath == null) {
      ScaffoldMessenger.of(context).showSnackBar(
        const SnackBar(content: Text('Please open a folder first')),
      );
      return;
    }

    // Create new document
    final filePath = '$folderPath/${template.name}';
    final document = Document(
      path: filePath,
      fileName: template.name,
      content: template.content,
      isDirty: true,
    );

    workspace.addDocument(document);
    workspace.setActiveDocument(document);
  }

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      appBar: AppBar(
        title: const Text('DevMark'),
        actions: [
          IconButton(
            icon: Icon(_viewMode == ViewMode.editor
                ? Icons.edit
                : _viewMode == ViewMode.split
                    ? Icons.view_column
                    : Icons.preview),
            onPressed: () {
              setState(() {
                _viewMode = ViewMode.values[(_viewMode.index + 1) % ViewMode.values.length];
              });
            },
            tooltip: 'Toggle view mode',
          ),
          IconButton(
            icon: Icon(_isDarkMode ? Icons.light_mode : Icons.dark_mode),
            onPressed: () {
              setState(() {
                _isDarkMode = !_isDarkMode;
              });
            },
            tooltip: 'Toggle theme',
          ),
          IconButton(
            icon: const Icon(Icons.save),
            onPressed: _saveDocument,
            tooltip: 'Save (Cmd/Ctrl+S)',
          ),
        ],
      ),
      drawer: Drawer(
        child: Column(
          children: [
            DrawerHeader(
              decoration: BoxDecoration(
                color: Theme.of(context).colorScheme.primaryContainer,
              ),
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Text(
                    'DevMark',
                    style: Theme.of(context).textTheme.headlineSmall,
                  ),
                  const SizedBox(height: 8),
                  Text(
                    'Markdown Editor for Developers',
                    style: Theme.of(context).textTheme.bodySmall,
                  ),
                ],
              ),
            ),
            ListTile(
              leading: const Icon(Icons.folder_open),
              title: const Text('Open Folder'),
              onTap: () {
                Navigator.pop(context);
                _openFolder();
              },
            ),
            ListTile(
              leading: const Icon(Icons.note_add),
              title: const Text('New from Template'),
              onTap: () {
                Navigator.pop(context);
                _newDocumentFromTemplate();
              },
            ),
            const Divider(),
            Expanded(
              child: _isLoading
                  ? const Center(child: CircularProgressIndicator())
                  : FileTree(
                      files: _markdownFiles,
                      selectedPath: context.watch<Workspace>().activeDocument?.path,
                      onFileSelected: (path) {
                        Navigator.pop(context);
                        _openFile(path);
                      },
                    ),
            ),
          ],
        ),
      ),
      body: Consumer<Workspace>(
        builder: (context, workspace, child) {
          final document = workspace.activeDocument;

          if (document == null) {
            return Center(
              child: Column(
                mainAxisAlignment: MainAxisAlignment.center,
                children: [
                  Icon(
                    Icons.description_outlined,
                    size: 64,
                    color: Colors.grey[400],
                  ),
                  const SizedBox(height: 16),
                  Text(
                    'No document open',
                    style: Theme.of(context).textTheme.titleLarge?.copyWith(
                          color: Colors.grey,
                        ),
                  ),
                  const SizedBox(height: 8),
                  ElevatedButton.icon(
                    onPressed: _openFolder,
                    icon: const Icon(Icons.folder_open),
                    label: const Text('Open Folder'),
                  ),
                ],
              ),
            );
          }

          return Column(
            children: [
              // Document tabs
              if (workspace.documents.length > 1)
                Container(
                  height: 40,
                  decoration: BoxDecoration(
                    color: Theme.of(context).colorScheme.surfaceVariant,
                    border: Border(
                      bottom: BorderSide(
                        color: Theme.of(context).dividerColor,
                      ),
                    ),
                  ),
                  child: ListView.builder(
                    scrollDirection: Axis.horizontal,
                    itemCount: workspace.documents.length,
                    itemBuilder: (context, index) {
                      final doc = workspace.documents[index];
                      final isActive = doc == document;

                      return Container(
                        decoration: BoxDecoration(
                          color: isActive
                              ? Theme.of(context).colorScheme.surface
                              : null,
                          border: Border(
                            right: BorderSide(
                              color: Theme.of(context).dividerColor,
                            ),
                          ),
                        ),
                        child: InkWell(
                          onTap: () => workspace.setActiveDocument(doc),
                          child: Padding(
                            padding: const EdgeInsets.symmetric(
                              horizontal: 16,
                              vertical: 8,
                            ),
                            child: Row(
                              children: [
                                Text(
                                  doc.fileName,
                                  style: TextStyle(
                                    fontWeight: isActive
                                        ? FontWeight.w600
                                        : FontWeight.normal,
                                  ),
                                ),
                                if (doc.isDirty)
                                  const Padding(
                                    padding: EdgeInsets.only(left: 4),
                                    child: Icon(Icons.circle, size: 8),
                                  ),
                                IconButton(
                                  icon: const Icon(Icons.close, size: 16),
                                  onPressed: () => workspace.removeDocument(doc),
                                  padding: const EdgeInsets.all(4),
                                  constraints: const BoxConstraints(
                                    minWidth: 24,
                                    minHeight: 24,
                                  ),
                                ),
                              ],
                            ),
                          ),
                        ),
                      );
                    },
                  ),
                ),
              // Editor/Preview area
              Expanded(
                child: _buildViewMode(document),
              ),
              // Status bar
              _buildStatusBar(document),
            ],
          );
        },
      ),
    );
  }

  Widget _buildViewMode(Document document) {
    switch (_viewMode) {
      case ViewMode.editor:
        return MarkdownEditor(
          document: document,
          isDarkMode: _isDarkMode,
        );
      case ViewMode.preview:
        return MarkdownPreview(
          document: document,
          isDarkMode: _isDarkMode,
        );
      case ViewMode.split:
        return Row(
          children: [
            Expanded(
              child: MarkdownEditor(
                document: document,
                isDarkMode: _isDarkMode,
              ),
            ),
            VerticalDivider(
              width: 1,
              color: Theme.of(context).dividerColor,
            ),
            Expanded(
              child: MarkdownPreview(
                document: document,
                isDarkMode: _isDarkMode,
              ),
            ),
          ],
        );
    }
  }

  Widget _buildStatusBar(Document document) {
    return Container(
      height: 24,
      padding: const EdgeInsets.symmetric(horizontal: 8),
      decoration: BoxDecoration(
        color: Theme.of(context).colorScheme.surfaceVariant,
        border: Border(
          top: BorderSide(
            color: Theme.of(context).dividerColor,
          ),
        ),
      ),
      child: Row(
        children: [
          Icon(
            document.isDirty ? Icons.circle : Icons.check_circle,
            size: 12,
            color: document.isDirty ? Colors.orange : Colors.green,
          ),
          const SizedBox(width: 4),
          Text(
            document.isDirty ? 'Modified' : 'Saved',
            style: const TextStyle(fontSize: 11),
          ),
          const SizedBox(width: 16),
          Text(
            document.fileName,
            style: const TextStyle(fontSize: 11),
          ),
          const Spacer(),
          Text(
            'Lines: ${document.content.split('\n').length}',
            style: const TextStyle(fontSize: 11),
          ),
        ],
      ),
    );
  }
}

/// Dialog for selecting a template
class _TemplateSelectionDialog extends StatelessWidget {
  final List<Template> templates;

  const _TemplateSelectionDialog({required this.templates});

  @override
  Widget build(BuildContext context) {
    return AlertDialog(
      title: const Text('New Document from Template'),
      content: SizedBox(
        width: 400,
        child: ListView.builder(
          shrinkWrap: true,
          itemCount: templates.length,
          itemBuilder: (context, index) {
            final template = templates[index];
            return ListTile(
              title: Text(template.name),
              subtitle: Text(template.description),
              onTap: () => Navigator.of(context).pop(template),
            );
          },
        ),
      ),
      actions: [
        TextButton(
          onPressed: () => Navigator.of(context).pop(),
          child: const Text('Cancel'),
        ),
      ],
    );
  }
}
