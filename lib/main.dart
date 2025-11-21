import 'package:flutter/material.dart';
import 'package:provider/provider.dart';
import 'models/workspace.dart';
import 'screens/main_shell.dart';

void main() {
  runApp(const DevMarkApp());
}

class DevMarkApp extends StatelessWidget {
  const DevMarkApp({super.key});

  @override
  Widget build(BuildContext context) {
    return ChangeNotifierProvider(
      create: (_) => Workspace(),
      child: MaterialApp(
        title: 'DevMark - Markdown Editor for Developers',
        debugShowCheckedModeBanner: false,
        theme: ThemeData(
          colorScheme: ColorScheme.fromSeed(
            seedColor: Colors.blue,
            brightness: Brightness.light,
          ),
          useMaterial3: true,
          fontFamily: 'Roboto',
        ),
        darkTheme: ThemeData(
          colorScheme: ColorScheme.fromSeed(
            seedColor: Colors.blue,
            brightness: Brightness.dark,
          ),
          useMaterial3: true,
          fontFamily: 'Roboto',
        ),
        themeMode: ThemeMode.system,
        home: const MainShell(),
      ),
    );
  }
}
