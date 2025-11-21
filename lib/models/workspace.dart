import 'package:flutter/foundation.dart';
import 'document.dart';

/// Manages a workspace with a folder and multiple documents
class Workspace extends ChangeNotifier {
  String? _folderPath;
  final List<Document> _documents = [];
  Document? _activeDocument;

  String? get folderPath => _folderPath;
  List<Document> get documents => List.unmodifiable(_documents);
  Document? get activeDocument => _activeDocument;

  bool get hasWorkspace => _folderPath != null;
  bool get hasDocuments => _documents.isNotEmpty;
  bool get hasDirtyDocuments => _documents.any((doc) => doc.isDirty);

  void setFolderPath(String? path) {
    _folderPath = path;
    notifyListeners();
  }

  void addDocument(Document document) {
    if (!_documents.contains(document)) {
      _documents.add(document);
      document.addListener(_onDocumentChanged);
      notifyListeners();
    }
  }

  void removeDocument(Document document) {
    document.removeListener(_onDocumentChanged);
    _documents.remove(document);
    if (_activeDocument == document) {
      _activeDocument = _documents.isNotEmpty ? _documents.last : null;
    }
    notifyListeners();
  }

  void setActiveDocument(Document? document) {
    if (document != null && !_documents.contains(document)) {
      addDocument(document);
    }
    _activeDocument = document;
    notifyListeners();
  }

  Document? getDocumentByPath(String path) {
    try {
      return _documents.firstWhere((doc) => doc.path == path);
    } catch (e) {
      return null;
    }
  }

  void closeAllDocuments() {
    for (var doc in _documents) {
      doc.removeListener(_onDocumentChanged);
    }
    _documents.clear();
    _activeDocument = null;
    notifyListeners();
  }

  void _onDocumentChanged() {
    notifyListeners();
  }

  @override
  void dispose() {
    for (var doc in _documents) {
      doc.removeListener(_onDocumentChanged);
    }
    super.dispose();
  }
}
