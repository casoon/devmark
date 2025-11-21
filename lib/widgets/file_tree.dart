import 'package:flutter/material.dart';
import 'package:path/path.dart' as path;

/// Represents a file in the tree
class FileTreeItem {
  final String name;
  final String fullPath;
  final bool isDirectory;
  final List<FileTreeItem> children;

  FileTreeItem({
    required this.name,
    required this.fullPath,
    this.isDirectory = false,
    this.children = const [],
  });
}

/// File tree widget for displaying markdown files in a directory
class FileTree extends StatefulWidget {
  final List<String> files;
  final String? selectedPath;
  final ValueChanged<String>? onFileSelected;

  const FileTree({
    super.key,
    required this.files,
    this.selectedPath,
    this.onFileSelected,
  });

  @override
  State<FileTree> createState() => _FileTreeState();
}

class _FileTreeState extends State<FileTree> {
  final Set<String> _expandedFolders = {};

  @override
  Widget build(BuildContext context) {
    if (widget.files.isEmpty) {
      return Center(
        child: Padding(
          padding: const EdgeInsets.all(16),
          child: Text(
            'No Markdown files found',
            style: Theme.of(context).textTheme.bodyMedium?.copyWith(
                  color: Colors.grey,
                ),
          ),
        ),
      );
    }

    return ListView(
      children: widget.files.map((filePath) {
        final fileName = path.basename(filePath);
        final isSelected = filePath == widget.selectedPath;

        return ListTile(
          dense: true,
          leading: const Icon(Icons.insert_drive_file, size: 18),
          title: Text(
            fileName,
            style: TextStyle(
              fontSize: 13,
              fontWeight: isSelected ? FontWeight.w600 : FontWeight.normal,
            ),
          ),
          subtitle: Text(
            filePath,
            style: const TextStyle(fontSize: 11),
            maxLines: 1,
            overflow: TextOverflow.ellipsis,
          ),
          selected: isSelected,
          onTap: () => widget.onFileSelected?.call(filePath),
        );
      }).toList(),
    );
  }
}
