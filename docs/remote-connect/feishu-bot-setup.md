# Feishu Bot Setup Guide

[中文](./feishu-bot-setup.zh-CN.md)

Use this guide to pair OpenBitFun through a Feishu bot.

## Receiving generated images and files

When a reply links to a generated file, the bot sends the attachment from that
reply's session workspace. Supported images appear as image messages; HTML,
documents, and other files arrive as file attachments. A preview image and a
link to the same file produce one attachment. If a file cannot be read or
uploaded, the bot reports the failure in the chat. Changing the selected
workspace while the agent runs does not change the source of its output files.

This also works with SSH workspaces and account-linked remote devices. After
a remote submission, the bot follows that turn and delivers its final reply and
attachments here. Questions and tool approvals can be answered in the bot chat.
A temporary disconnect resumes polling the same turn without submitting it again.
Attachment size limits imposed by the target and chat provider still apply.

## Setup Steps

### Step 1

Open the Feishu Developer Platform and log in:

<https://open.feishu.cn/app?lang=en-US>

### Step 2

Create a custom app.

### Step 3

Add the bot feature:

Features - Bot - Add

### Step 4

Add permission scopes:

Permissions & Scopes - Add Scopes - Search for `im:` - Select all scopes that do not require approval - Add Scopes

### Step 5

Copy the app credentials:

Credentials & Basic Info - App ID and App Secret

### Step 6

Open OpenBitFun and start the Feishu bot connection:

Remote Connect - IM Bot - Feishu Bot - Fill in App ID and App Secret - Connect

### Step 7

Return to the Feishu Developer Platform.

### Step 8

Configure event subscriptions:

Events & callbacks - Event configuration - Subscription mode - Persistent connection - Save

Then add message events:

Add Events - Search for `im.message` - Select all - Confirm

### Step 9

Configure callback subscriptions:

Events & callbacks - Callback configuration - Subscription mode - Persistent connection - Save

Then add card action callbacks:

Add callback - Search for `card.action.trigger` - Select it - Confirm

### Step 10

Publish the bot.

### Step 11

Open Feishu, search for the bot name, open the chat, enter any message, and send it.

### Step 12

Enter the 6-digit pairing code shown in OpenBitFun Desktop, send it, and wait for the connection to succeed.
