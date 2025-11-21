import 'dart:io';
import 'package:path/path.dart' as path;
import 'package:flutter/services.dart' show rootBundle;

/// Template types for developer documentation
enum TemplateType {
  readme,
  changelog,
  adr,
  apiDoc,
  contributing,
  codeOfConduct,
  license,
}

/// Information about a template
class Template {
  final TemplateType type;
  final String name;
  final String description;
  final String content;

  Template({
    required this.type,
    required this.name,
    required this.description,
    required this.content,
  });
}

/// Service for managing document templates
class TemplateService {
  final Map<TemplateType, Template> _templates = {};

  /// Initialize templates with default content
  Future<void> initialize() async {
    _templates[TemplateType.readme] = Template(
      type: TemplateType.readme,
      name: 'README.md',
      description: 'Standard project README',
      content: _readmeTemplate,
    );

    _templates[TemplateType.changelog] = Template(
      type: TemplateType.changelog,
      name: 'CHANGELOG.md',
      description: 'Keep a changelog format',
      content: _changelogTemplate,
    );

    _templates[TemplateType.adr] = Template(
      type: TemplateType.adr,
      name: 'ADR.md',
      description: 'Architecture Decision Record',
      content: _adrTemplate,
    );

    _templates[TemplateType.apiDoc] = Template(
      type: TemplateType.apiDoc,
      name: 'API.md',
      description: 'API Documentation',
      content: _apiDocTemplate,
    );

    _templates[TemplateType.contributing] = Template(
      type: TemplateType.contributing,
      name: 'CONTRIBUTING.md',
      description: 'Contributing guidelines',
      content: _contributingTemplate,
    );

    _templates[TemplateType.codeOfConduct] = Template(
      type: TemplateType.codeOfConduct,
      name: 'CODE_OF_CONDUCT.md',
      description: 'Code of Conduct',
      content: _codeOfConductTemplate,
    );

    _templates[TemplateType.license] = Template(
      type: TemplateType.license,
      name: 'LICENSE.md',
      description: 'MIT License',
      content: _licenseTemplate,
    );
  }

  /// Get all available templates
  List<Template> getAllTemplates() {
    return _templates.values.toList();
  }

  /// Get a specific template by type
  Template? getTemplate(TemplateType type) {
    return _templates[type];
  }

  /// README.md template
  static const String _readmeTemplate = '''# Project Name

## Description

A brief description of your project.

## Features

- Feature 1
- Feature 2
- Feature 3

## Installation

\`\`\`bash
# Installation instructions
npm install
\`\`\`

## Usage

\`\`\`javascript
// Usage example
const example = require('example');
\`\`\`

## Documentation

For detailed documentation, see [docs/](./docs/).

## Contributing

Please read [CONTRIBUTING.md](CONTRIBUTING.md) for details on our code of conduct and the process for submitting pull requests.

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.
''';

  /// CHANGELOG.md template
  static const String _changelogTemplate = '''# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- New features

### Changed
- Changes in existing functionality

### Deprecated
- Soon-to-be removed features

### Removed
- Removed features

### Fixed
- Bug fixes

### Security
- Security fixes

## [1.0.0] - YYYY-MM-DD

### Added
- Initial release
''';

  /// ADR template
  static const String _adrTemplate = '''# ADR-NNNN: Title

## Status

Proposed | Accepted | Deprecated | Superseded

## Context

What is the issue that we're seeing that is motivating this decision or change?

## Decision

What is the change that we're proposing and/or doing?

## Consequences

What becomes easier or more difficult to do because of this change?

### Positive

- Benefit 1
- Benefit 2

### Negative

- Trade-off 1
- Trade-off 2

## Alternatives Considered

What other options were considered?

## References

- Related documents
- Links to discussions
''';

  /// API Documentation template
  static const String _apiDocTemplate = '''# API Documentation

## Base URL

\`\`\`
https://api.example.com/v1
\`\`\`

## Authentication

All requests require authentication using Bearer tokens:

\`\`\`bash
Authorization: Bearer YOUR_TOKEN
\`\`\`

## Endpoints

### GET /resource

Get a list of resources.

**Parameters:**

- \`limit\` (optional): Number of items to return (default: 10)
- \`offset\` (optional): Number of items to skip (default: 0)

**Response:**

\`\`\`json
{
  "data": [
    {
      "id": "123",
      "name": "Example"
    }
  ],
  "total": 100
}
\`\`\`

### POST /resource

Create a new resource.

**Request Body:**

\`\`\`json
{
  "name": "New Resource",
  "description": "Description here"
}
\`\`\`

**Response:**

\`\`\`json
{
  "id": "456",
  "name": "New Resource",
  "createdAt": "2024-01-01T00:00:00Z"
}
\`\`\`

## Error Codes

- \`400\` - Bad Request
- \`401\` - Unauthorized
- \`404\` - Not Found
- \`500\` - Internal Server Error
''';

  /// CONTRIBUTING.md template
  static const String _contributingTemplate = '''# Contributing

Thank you for your interest in contributing to this project!

## How to Contribute

### Reporting Bugs

- Use the issue tracker
- Describe the bug in detail
- Include steps to reproduce
- Mention your environment (OS, versions, etc.)

### Suggesting Features

- Use the issue tracker
- Clearly describe the feature
- Explain why it would be useful
- Consider providing examples

### Pull Requests

1. Fork the repository
2. Create a new branch (\`git checkout -b feature/amazing-feature\`)
3. Make your changes
4. Write or update tests
5. Ensure all tests pass
6. Commit your changes (\`git commit -m 'Add amazing feature'\`)
7. Push to the branch (\`git push origin feature/amazing-feature\`)
8. Open a Pull Request

## Code Style

Please follow the existing code style and conventions.

## Testing

Make sure to add tests for new features and bug fixes.

\`\`\`bash
# Run tests
npm test
\`\`\`

## License

By contributing, you agree that your contributions will be licensed under the project's license.
''';

  /// CODE_OF_CONDUCT.md template
  static const String _codeOfConductTemplate = '''# Code of Conduct

## Our Pledge

We pledge to make participation in our project a harassment-free experience for everyone.

## Our Standards

Examples of behavior that contributes to a positive environment:

- Using welcoming and inclusive language
- Being respectful of differing viewpoints
- Gracefully accepting constructive criticism
- Focusing on what is best for the community

Examples of unacceptable behavior:

- Trolling, insulting/derogatory comments, and personal attacks
- Public or private harassment
- Publishing others' private information without permission
- Other conduct which could reasonably be considered inappropriate

## Enforcement

Project maintainers are responsible for clarifying standards of acceptable behavior and may take action in response to unacceptable behavior.

## Attribution

This Code of Conduct is adapted from the [Contributor Covenant](https://www.contributor-covenant.org/).
''';

  /// LICENSE template (MIT)
  static const String _licenseTemplate = '''MIT License

Copyright (c) [year] [fullname]

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
''';
}
