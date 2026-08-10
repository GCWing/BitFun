import { MUTED } from './Theme';

@ComponentV2
export struct ToolGlyph {
  @Param kind: string = 'tool';
  @Param color: ResourceColor = MUTED;

  build() {
    SymbolGlyph(this.symbol())
      .fontSize(this.isChevron() ? 18 : 14)
      .fontColor([this.color])
      .width(this.isChevron() ? 14 : 15)
      .height(this.isChevron() ? 14 : 15)
  }

  private isChevron(): boolean {
    return this.kind.indexOf('chevron_') === 0;
  }

  private symbol(): Resource {
    if (this.kind === 'search') return $r('sys.symbol.magnifyingglass');
    if (this.kind === 'document') return $r('sys.symbol.doc_text');
    if (this.kind === 'stack') return $r('sys.symbol.rectangle_stack');
    if (this.kind === 'question') return $r('sys.symbol.questionmark_circle');
    if (this.kind === 'todo') return $r('sys.symbol.list_checkmark');
    if (this.kind === 'task') return $r('sys.symbol.robot');
    if (this.kind === 'git') return $r('sys.symbol.arrow_triangle_merge');
    if (this.kind === 'delete') return $r('sys.symbol.trash');
    if (this.kind === 'diff') return $r('sys.symbol.doc_text_badge_magnifyingglass');
    if (this.kind === 'patch' || this.kind === 'command') return $r('sys.symbol.code_square');
    if (this.kind === 'create') return $r('sys.symbol.doc_text_badge_arrow_up');
    if (this.kind === 'mutate') return $r('sys.symbol.square_and_pencil');
    if (this.kind === 'folder') return $r('sys.symbol.folder');
    if (this.kind === 'web') return $r('sys.symbol.link');
    if (this.kind === 'chevron_right') return $r('sys.symbol.chevron_right');
    if (this.kind === 'chevron_up') return $r('sys.symbol.chevron_up');
    if (this.kind === 'chevron_down') return $r('sys.symbol.chevron_down');
    return $r('sys.symbol.wrench_and_screwdriver');
  }
}
