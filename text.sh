#!/bin/bash

# DevMark Starter Script
# Startet die DevMark Flutter Anwendung

# Farben für Output
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
NC='\033[0m' # No Color

echo -e "${GREEN}=====================================${NC}"
echo -e "${GREEN}  DevMark - Markdown Editor Starter${NC}"
echo -e "${GREEN}=====================================${NC}"
echo ""

# Prüfe ob Flutter installiert ist
if ! command -v flutter &> /dev/null
then
    echo -e "${RED}Flutter ist nicht installiert oder nicht im PATH!${NC}"
    echo -e "${YELLOW}Bitte installiere Flutter von: https://flutter.dev${NC}"
    exit 1
fi

echo -e "${GREEN}✓ Flutter gefunden${NC}"
echo ""

# Prüfe ob Dependencies installiert sind
if [ ! -d ".dart_tool" ]; then
    echo -e "${YELLOW}Dependencies werden installiert...${NC}"
    flutter pub get
    echo ""
fi

# Ermittle verfügbare Geräte
echo -e "${YELLOW}Verfügbare Geräte:${NC}"
flutter devices
echo ""

# Starte die Anwendung auf macOS (da wir auf Darwin sind)
echo -e "${GREEN}Starte DevMark auf macOS...${NC}"
echo ""
flutter run -d macos
