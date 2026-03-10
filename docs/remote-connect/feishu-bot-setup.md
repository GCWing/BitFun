# Feishu Bot Setup Guide

[中文](./feishu-bot-setup.zh-CN.md)

Use this guide to pair BitFun through a Feishu bot.

## Setup Steps

### Step1

Open the Feishu Developer Platform and log in

<https://open.feishu.cn/app?lang=en-US>

### Step2

Create custom app

![Create custom app](./images/English/1.png)

### Step3

Add Features - Bot - Add

![Add bot](./images/English/2.png)

### Step4

Permissions & Scopes -

Add permission scopes to app -

Search "im:" - Approval required "No" - Select all - Add Scopes

![Add permission scopes to app](./images/English/3.png)

### Step5

Credentials & Basic Info - Copy App ID and App Secret

![Credentials & Basic Info](./images/English/4.png)

### Step6

Open BitFun - Remote Connect - SMS Bot - Feishu Bot - Fill in App ID and App Secret - Connect

![Connect 1](./images/English/5.png)
![Connect 2](./images/English/6.png)

### Step7

Back to Feishu Developer Platform

### Step8

Events & callbacks - Event configuration -

Subscription mode - persistent connection - Save

Add Events - Search "im.message" - Select all - Confirm

![Event configuration 1](./images/English/7.png)
![Event configuration 2](./images/English/8.png)

### Step9

Events & callbacks - Callback configuration -

Subscription mode - persistent connection - Save

Add callback - Search "card.action.trigger" - Select all - Confirm

![Callback configuration 1](./images/English/9.png)
![Callback configuration 2](./images/English/10.png)

### Step10

Publish the bot

![Publish the bot 1](./images/English/11.png)
![Publish the bot 2](./images/English/12.png)
![Publish the bot 3](./images/English/13.png)
![Publish the bot 4](./images/English/14.png)

### Step11

Open Feishu - Search "{robot name}" -

Click the robot to open the chat box - Input any message and send

### Step12

Enter the 6-digit pairing code from BitFun Desktop - Send - Connection successful

![Verification](./images/English/15.png)