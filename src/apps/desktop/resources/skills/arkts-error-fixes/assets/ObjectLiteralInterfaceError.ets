interface Article {
  title: string;
  desc: string;
  image: Resource;
}

@Entry
@Component
struct ObjectLiteralInterfaceError {
  private articles: Article[] = [
    { title: '文章1', desc: '这是文章1的描述', image: $r('app.media.article1') },
    { title: '文章2', desc: '这是文章2的描述', image: $r('app.media.article2') },
    { title: '文章3', desc: '这是文章3的描述', image: $r('app.media.article3') }
  ];

  build() {
    Scroll() {
      Column() {
        ForEach(this.articles, (article: Article, index: number) => {
          Column() {
            Image(article.image)
              .width('100%')
              .height(200)
              .objectFit(ImageFit.Cover)
              .borderRadius(8)
            
            Text(article.title)
              .fontSize('20fp')
              .fontWeight(FontWeight.Bold)
              .margin({ top: 8 })
            
            Text(article.desc)
              .fontSize('14fp')
              .fontColor('#666666')
              .margin({ top: 4 })
          }
          .padding(16)
          .backgroundColor('#FFFFFF')
          .borderRadius(8)
          .margin({ bottom: 16 })
        })
      }
      .padding({ left: 16, right: 16, top: 16, bottom: 16 })
    }
    .width('100%')
    .height('100%')
    .backgroundColor('#F5F5F5')
  }
}
