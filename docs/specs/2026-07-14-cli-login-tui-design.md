# CLI Login TUI Design

## Problem

`/login` temporarily disables raw mode and uses `print!` + echo-off stdin prompts. That fights the ratatui alternate screen and produces misaligned / overlapping characters.

## Decision

Dedicated full-viewport Login page (approach A), ratatui-native, no cooked-mode prompts.

## Behavior

- Entry: `/login` or palette Login from startup or chat.
- Fields start empty (no credential-hint / default relay prefill).
- Focus cycle (Up/Down, Tab/Shift+Tab): Auth Server → Username → Password → Login.
- Password stores plaintext; display is `*` repeated by character count.
- Enter on a text field moves focus down; Enter on Login submits.
- Esc cancels and returns to the previous screen.
- Validation / API errors stay on the form; success hides the form and shows a status/system message.
- Persistence and Peer Host routing reuse existing `account` APIs after credentials are collected.

## Non-goals

- Changing Desktop login UI
- Changing Auth Server protocol
- Prefill / remembered credentials on the form fields
