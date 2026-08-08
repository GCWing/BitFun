import { ConversationUiQuestionAnswer } from '../components/ConversationUiModels';
import { FilePreviewRequest } from '../../model/FilePreviewTarget';

export enum ConversationIntentType {
  OpenSidebar = 'open_sidebar',
  Back = 'back',
  NewSession = 'new_session',
  TogglePin = 'toggle_pin',
  Archive = 'archive',
  Delete = 'delete',
  ShowUploadedFiles = 'show_uploaded_files',
  Stop = 'stop',
  LoadOlder = 'load_older',
  ApproveTool = 'approve_tool',
  RejectTool = 'reject_tool',
  CancelTool = 'cancel_tool',
  AnswerQuestion = 'answer_question',
  RenameSession = 'rename_session',
  CopyMessage = 'copy_message',
  RetryMessage = 'retry_message',
  SelectModel = 'select_model',
  PickImages = 'pick_images',
  RemoveImage = 'remove_image',
  OpenFilePreview = 'open_file_preview',
  DownloadFile = 'download_file',
  Send = 'send',
  VoiceInput = 'voice_input',
  ChatInputChanged = 'chat_input_changed'
}

export class ConversationIntent {
  readonly type: ConversationIntentType;
  readonly value: string;
  readonly toolId: string;
  readonly updatedInput?: Object;
  readonly answers?: ConversationUiQuestionAnswer;
  readonly filePreviewRequest?: FilePreviewRequest;

  constructor(
    type: ConversationIntentType,
    value: string = '',
    toolId: string = '',
    updatedInput?: Object,
    answers?: ConversationUiQuestionAnswer,
    filePreviewRequest?: FilePreviewRequest
  ) {
    this.type = type;
    this.value = value;
    this.toolId = toolId;
    this.updatedInput = updatedInput;
    this.answers = answers;
    this.filePreviewRequest = filePreviewRequest;
  }
}

export class ConversationIntents {
  static simple(type: ConversationIntentType): ConversationIntent {
    return new ConversationIntent(type);
  }

  static value(type: ConversationIntentType, value: string): ConversationIntent {
    return new ConversationIntent(type, value);
  }

  static approveTool(toolId: string, updatedInput?: Object): ConversationIntent {
    return new ConversationIntent(ConversationIntentType.ApproveTool, '', toolId, updatedInput);
  }

  static tool(type: ConversationIntentType, toolId: string): ConversationIntent {
    return new ConversationIntent(type, '', toolId);
  }

  static answerQuestion(toolId: string, answers: ConversationUiQuestionAnswer): ConversationIntent {
    return new ConversationIntent(ConversationIntentType.AnswerQuestion, '', toolId, undefined, answers);
  }

  static openFilePreview(reference: string, label: string): ConversationIntent {
    return new ConversationIntent(
      ConversationIntentType.OpenFilePreview,
      '',
      '',
      undefined,
      undefined,
      new FilePreviewRequest(reference, label)
    );
  }
}
