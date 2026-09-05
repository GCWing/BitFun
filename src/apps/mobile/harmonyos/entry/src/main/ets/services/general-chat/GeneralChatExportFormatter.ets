import { ChatMessage, RemoteSession } from '../../model/RemoteModels';

export class GeneralChatExportFormatter {
  static markdown(session: RemoteSession, messages: ChatMessage[]): string {
    const title = session.title.trim().length > 0 ? session.title.trim() : '未命名会话';
    const firstUserContent = GeneralChatExportFormatter.firstVisibleUserContent(messages);
    const sections: string[] = [];
    if (!GeneralChatExportFormatter.repeatsFirstUserMessage(title, firstUserContent)) {
      sections.push(`# ${title}`);
    }
    messages.forEach((message: ChatMessage) => {
      const content = message.text.trim();
      if (content.length === 0) {
        return;
      }
      sections.push(`## ${GeneralChatExportFormatter.roleLabel(message.role)}\n\n${content}`);
    });
    return `${sections.join('\n\n')}\n`;
  }

  private static firstVisibleUserContent(messages: ChatMessage[]): string {
    for (let index = 0; index < messages.length; index += 1) {
      const message = messages[index];
      if (message.role === 'user' && message.text.trim().length > 0) {
        return message.text.trim();
      }
    }
    return '';
  }

  private static repeatsFirstUserMessage(title: string, firstUserContent: string): boolean {
    if (firstUserContent.length === 0) {
      return false;
    }
    return GeneralChatExportFormatter.comparableHeading(title) ===
      GeneralChatExportFormatter.comparableHeading(firstUserContent);
  }

  private static comparableHeading(value: string): string {
    return value
      .trim()
      .replace(/\s+/g, ' ')
      .replace(/[。！？!?…，,；;：:]+$/g, '')
      .trim();
  }

  private static roleLabel(role: string): string {
    return role === 'user' ? '用户' : 'OpenBitFun';
  }
}
