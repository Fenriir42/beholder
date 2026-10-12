---
title: Getting started
description: Install Beholder, connect to your group's server and create your account.
sidebar:
  order: 1
---

## Install the app

Beholder runs on **Windows** and **Linux**.

- **Windows:** run the `Beholder_x.y.z_x64-setup.exe` installer. If your PC doesn't have Microsoft WebView2 yet, the installer downloads it (most Windows 10 and 11 PCs already have it).
- **Debian, Ubuntu, Mint:** install the `.deb` package (`sudo apt install ./beholder_x.y.z_amd64.deb`).
- **Fedora, openSUSE:** install the `.rpm` package (`sudo dnf install ./beholder-x.y.z-1.x86_64.rpm`).

Your DM or whoever runs the server will tell you where to get these files.

## Connect to the server

The first time you open Beholder, it asks for the **server address**: something like `https://dnd.example.org`, given by whoever runs your group's server. You can type it without `https://`.

Beholder remembers the address. To change it later, use **Change server** on the login screen.

## Create your account

1. Choose **Create account**.
2. Pick a **username** (3 to 32 letters, digits, `_`, `-` or `.`; it's how you log in) and a **display name** (what other players see).
3. Choose a password of at least 8 characters.

The **Continue with Discord** button is shown but disabled: Discord login comes in a later update, and password login will always keep working, because Discord isn't reachable everywhere.

## Staying logged in

Beholder keeps your session in your system's password store (Windows Credential Manager, or the GNOME Keyring / KWallet on Linux), so you don't have to log in every time.

If your Linux desktop has no password store, Beholder still works but forgets the session when you close it, so you log in again on the next launch.

## Language

The **EN / FR** button in the top bar switches the whole interface between English and French, instantly. Beholder remembers your choice.
