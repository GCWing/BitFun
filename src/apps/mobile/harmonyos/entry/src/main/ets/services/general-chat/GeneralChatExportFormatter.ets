import { ChatMessage, RemoteSession } from '../../model/RemoteModels';

export class GeneralChatExportFormatter {
  static markdown(session: RemoteSession, messages: ChatMessage[]): string {
    const title = session.title.trim().length > 0 ? session.title.trim() : '未命名会话';
    const sections: string[] = [`# ${title}`];
    messages.forEach((message: ChatMessage) => {
      const content = message.text.trim();
      if (content.length === 0) {
        return;
      }
      sections.push(`## ${GeneralChatExportFormatter.roleLabel(message.role)}\n\n${content}`);
    });
    return `${sections.join('\n\n')}\n`;
  }

  private static roleLabel(role: string): string {
    return role === 'user' ? '用户' : 'BitFun';
  }
}
