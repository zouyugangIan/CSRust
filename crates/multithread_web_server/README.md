# 宝宝的生日小站

在仓库根目录运行：

```bash
cargo run -p multithread_web_server
```

浏览器打开 <http://127.0.0.1:8877>。修改 Rust 代码后需要按 Ctrl+C 停掉旧服务，再重新运行；只修改 HTML 或照片则刷新浏览器即可。

- 首页与生日祝福：`babyHappyBirthDay.html`，样式也写在这个文件的 `<style>` 中。
- 两张照片：`images/1.jpg`、`images/2.jpg`。用同名 JPG 文件替换即可，照片直接展示，点击不会跳转。
- 页面适配电脑和手机，没有外部字体、图片或 JavaScript 依赖。
- 视频：`images/3.MP4`（注意扩展名是大写），在页面下方直接播放，支持暂停、音量、全屏和拖动进度。

服务器保持运行直到按 Ctrl+C，保留原来的 `/hello.html`、`/sleep` 教学路由。默认只监听本机。
