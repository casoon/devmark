import 'package:flutter/foundation.dart';

/// Represents a Markdown document with its content, path, and dirty state
class Document extends ChangeNotifier {
  String _content;
  final String path;
  final String fileName;
  bool _isDirty;
  DateTime _lastModified;

  Document({
    required this.path,
    required this.fileName,
    String content = '',
    bool isDirty = false,
  })  : _content = content,
        _isDirty = isDirty,
        _lastModified = DateTime.now();

  String get content => _content;
  bool get isDirty => _isDirty;
  DateTime get lastModified => _lastModified;

  set content(String newContent) {
    if (_content != newContent) {
      _content = newContent;
      _isDirty = true;
      _lastModified = DateTime.now();
      notifyListeners();
    }
  }

  void markClean() {
    _isDirty = false;
    notifyListeners();
  }

  void updateContent(String newContent, {bool markDirty = true}) {
    _content = newContent;
    if (markDirty) {
      _isDirty = true;
    }
    _lastModified = DateTime.now();
    notifyListeners();
  }

  @override
  String toString() => 'Document(path: $path, isDirty: $_isDirty)';
}
