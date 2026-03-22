# Feishu Bot Setup Guide

[中文](./feishu-bot-setup.zh-CN.md)

Use this guide to pair BitFun through a Feishu bot.

## Setup Steps

### Step1

Open the Feishu Developer Platform and log in

<https://open.feishu.cn/app?lang=en-US>

### Step2

Create custom app

### Step3

Add Features - Bot - Add

### Step4

Permissions & Scopes -

Add permission scopes to app -

Search "im:" - Approval required "No" - Select all - Add Scopes

### Step5

Credentials & Basic Info - Copy App ID and App Secret

### Step6

Open BitFun - Remote Connect - IM Bot - Feishu Bot - Fill in App ID and App Secret - Connect

### Step7

Back to Feishu Developer Platform

### Step8

Events & callbacks - Event configuration -

Subscription mode - persistent connection - Save

Add Events - Search "im.message" - Select all - Confirm

### Step9

Events & callbacks - Callback configuration -

Subscription mode - persistent connection - Save

Add callback - Search "card.action.trigger" - Select all - Confirm

### Step10

Publish the bot

### Step11

Open Feishu - Search "{robot name}" -

Click the robot to open the chat box - Input any message and send

### Step12

Enter the 6-digit pairing code from BitFun Desktop - Send - Connection successful
