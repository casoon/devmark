import 'dart:io';
import 'package:path/path.dart' as path;
import '../models/document.dart';

/// Service for file system operations
class FileService {
  /// Read a file and return its content as a string
  Future<String> readFile(String filePath) async {
    try {
      final file = File(filePath);
      return await file.readAsString();
    } catch (e) {
      throw Exception('Failed to read file: $e');
    }
  }

  /// Write content to a file
  Future<void> writeFile(String filePath, String content) async {
    try {
      final file = File(filePath);
      await file.writeAsString(content);
    } catch (e) {
      throw Exception('Failed to write file: $e');
    }
  }

  /// Save a document to disk
  Future<void> saveDocument(Document document) async {
    await writeFile(document.path, document.content);
    document.markClean();
  }

  /// Load a document from disk
  Future<Document> loadDocument(String filePath) async {
    final content = await readFile(filePath);
    final fileName = path.basename(filePath);
    return Document(
      path: filePath,
      fileName: fileName,
      content: content,
      isDirty: false,
    );
  }

  /// List all Markdown files in a directory
  Future<List<String>> listMarkdownFiles(String directoryPath) async {
    try {
      final directory = Directory(directoryPath);
      final files = <String>[];

      await for (var entity in directory.list(recursive: true)) {
        if (entity is File && _isMarkdownFile(entity.path)) {
          files.add(entity.path);
        }
      }

      files.sort();
      return files;
    } catch (e) {
      throw Exception('Failed to list files: $e');
    }
  }

  /// Check if a file is a Markdown file based on extension
  bool _isMarkdownFile(String filePath) {
    final extension = path.extension(filePath).toLowerCase();
    return extension == '.md' || extension == '.markdown' || extension == '.mdx';
  }

  /// Check if a file exists
  Future<bool> fileExists(String filePath) async {
    return await File(filePath).exists();
  }

  /// Create a new file with optional content
  Future<void> createFile(String filePath, {String content = ''}) async {
    try {
      final file = File(filePath);

      // Create parent directories if they don't exist
      await file.parent.create(recursive: true);

      await file.writeAsString(content);
    } catch (e) {
      throw Exception('Failed to create file: $e');
    }
  }

  /// Delete a file
  Future<void> deleteFile(String filePath) async {
    try {
      final file = File(filePath);
      await file.delete();
    } catch (e) {
      throw Exception('Failed to delete file: $e');
    }
  }
}
