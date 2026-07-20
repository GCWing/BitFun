import pasteboard from '@ohos.pasteboard';

export class ClipboardService {
  async readText(): Promise<string> {
    const systemPasteboard = pasteboard.getSystemPasteboard();
    const hasData = await systemPasteboard.hasData();
    if (!hasData) {
      return '';
    }

    const data = await systemPasteboard.getData();
    const recordCount = data.getRecordCount();
    for (let index = 0; index < recordCount; index++) {
      const record = data.getRecord(index);
      const plainText = (record.plainText || '').trim();
      if (plainText.length > 0) {
        return plainText;
      }
      const uri = (record.uri || '').trim();
      if (uri.length > 0) {
        return uri;
      }
    }

    return '';
  }

  async writeText(text: string): Promise<void> {
    const data = pasteboard.createData(pasteboard.MIMETYPE_TEXT_PLAIN, text);
    await pasteboard.getSystemPasteboard().setData(data);
  }
}
