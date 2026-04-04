# Rust -> AI 行动方案总结（2026-04 执行版）

## 1. 文档定位

这不是一份泛泛而谈的路线图，也不是一份只会堆书单的收藏夹。

这是一份可执行的行动方案，目标是把你当前这个 Rust workspace，逐步推进成：

- 一个真正能承载长期学习与项目演进的 Rust 工程仓库
- 一条从 Rust 基础到 Rust 工程，再到 Rust AI 的成长路径
- 一组能形成作品集的阶段性项目，而不是散乱 demo

核心要求只有一句话：

`稳中求快`

也就是：

- 不能一开始就冲太深，导致基础不稳、项目烂尾
- 也不能无限补基础，始终不进入工程与 AI 场景

正确做法是：每个阶段只抓主线、持续交付、不断把学习沉淀回这个仓库。

## 2. 先说结论

如果只保留一条最优主线，我建议这样走：

`Rust 语言与工程基本功 -> Rust 工程化与服务端 -> AI 基础闭环 -> Rust AI 应用与推理工程 -> 选择一个高价值方向做深`

这条线比“先把全部传统 CS 补完再做 AI”更快，也比“直接冲 LLM 热点”更稳。

2026-04 这个时间点下，最现实的判断是：

- Rust 应该是你的长期主战语言
- AI 不应该只理解为“训练模型”
- 对你最有成长性的组合，是 `Rust 负责工程主干 + 适度借用 Python/Hub/现成模型生态`

一句话总结：

先成为强 Rust 工程师，再成为懂 AI 系统的 Rust 工程师。

## 3. 当前仓库的现实起点

从仓库现状看，你已经跨过了最开始的“不会写 Rust”阶段：

- 已有 Cargo workspace
- 已有 `playground`、`guessing_game`、`todo_cli`
- 已能完成基础语法和小工具练习

但下一阶段的缺口也很明显：

- 语言能力还没有到“稳定设计数据与模块边界”
- 工程能力还没有形成测试、错误处理、日志、配置、分层习惯
- 服务能力还没有进入 `tokio` / `axum` / `sqlx`
- AI 能力还没有形成 `数据 -> 检索/推理 -> 评测 -> 服务化` 的闭环

所以现在最该做的，不是继续堆更多零散示例，而是把仓库升级成一条递进式成长路径。

## 4. 总路线图

默认节奏按每周 12-15 小时设计，这是“稳中求快”的推荐区间。

| 阶段 | 时长 | 主目标 | 关键产出 |
| --- | ---: | --- | --- |
| 0. 地基重置 | 1-2 周 | 建立 2026 风格的 Rust 工程底座 | 统一工具链、质量门槛、学习节奏 |
| 1. Rust 核心深化 | 4-6 周 | 把语言能力从“能写”升级到“能设计” | 2-3 个主题 lab + 1 个像样的 CLI |
| 2. 工程化进阶 | 4-6 周 | 建立测试、错误处理、日志、文档习惯 | 可维护的中小型 crate |
| 3. 服务与系统能力 | 6-8 周 | 进入 async/http/db/并发/性能基础 | 1 个可运行 API 服务 |
| 4. AI 基础闭环 | 6-8 周 | 打通 tokenizer/embedding/retrieval/eval 概念链 | 1 个检索或 embedding demo |
| 5. Rust AI 实战 | 6-8 周 | 做出完整 AI 应用最小闭环 | 1 个可演示 Rust AI MVP |
| 6. 高成长专精 | 8-12 周 | 形成自己的技术护城河 | 1 个可进入作品集的代表项目 |

如果你每周只有 8-10 小时，这条路线拉长到 12-15 个月更合理。

如果你每周能稳定投入 16-20 小时，可以压缩到 7-9 个月，但不建议同时开太多线。

## 5. 资源使用原则

### 5.1 每个阶段只保留三类主资源

每个阶段只保留：

- 1 组主文档
- 1 个主仓库
- 1 本主书或主课程

否则你会掉进最典型的坑：

- 文档看很多，但没有主线
- 仓库 clone 很多，但没读进去
- 书买很多，但没有项目产出

### 5.2 文档、仓库、书分别怎么用

- 文档：用来建立正确 API 心智和最佳实践，不要只看二手博客
- 仓库：用来学习目录结构、测试写法、模块边界、真实代码风格
- 书：用来补系统性和深层解释，不负责替代项目实践

### 5.3 不要“平行推很多资源”

正确节奏是：

`一本主书/课程 + 一组官方文档 + 一个主仓库 + 你自己的阶段项目`

### 5.4 读仓库的正确姿势

不是一上来通读全部代码，而是按下面顺序：

1. 先看 README / docs / examples
2. 再看 `Cargo.toml` 与 crate 划分
3. 再看 `src/lib.rs` / `src/main.rs` / `bin/`
4. 再看 tests / examples
5. 最后再深入内部实现

### 5.5 书不是全都要从头读到尾

不同资源定位不同：

- 入门书：按顺序读
- 中级书：按主题挑章节
- 仓库：按问题驱动去读
- 官方文档：按项目需求反复查

## 6. 阶段执行方案与具体资源

下面这一节回答两个问题：

- 具体怎么做
- 学哪些仓库、文档或书

---

### 阶段 0：地基重置（1-2 周）

#### 目标

- 统一 Rust 2024 edition 心智
- 建立稳定开发环境
- 把这个仓库从“练习堆积”改成“阶段成长容器”

#### 必学主题

- `rustup`、toolchain、stable/beta/nightly 的区别
- Cargo workspace
- `cargo fmt`、`cargo clippy`、`cargo test`
- Rust 2024 edition 的基本变化
- IDE 与 `rust-analyzer`

#### 主文档

- [The Rust Programming Language](https://doc.rust-lang.org/book/title-page.html)
  - 重点先读第 1-7 章和附录 D
- [The Cargo Book](https://doc.rust-lang.org/cargo/)
- [Rust 2024 Edition Guide](https://doc.rust-lang.org/edition-guide/rust-2024/index.html)
- [Rust release notes](https://doc.rust-lang.org/stable/releases.html)
  - 重点只看 1.85.0 起的 edition 相关变化即可

#### 主仓库

- [rust-lang/rustlings](https://github.com/rust-lang/rustlings)

#### 主书

- 主书就是 [The Rust Programming Language](https://doc.rust-lang.org/book/title-page.html)
  - 纸质版可选，但官方在线版优先

#### 执行方式

1. 先更新本机 Rust stable，明确后续项目默认按 `edition = "2024"` 思路写
2. 用 3-5 天快速复习 Rust Book 的前半部分，不做重笔记，只做自己的关键错点记录
3. 并行做 Rustlings，重点不是刷完，而是暴露自己在 ownership、enum、error handling 上的真实薄弱点
4. 回到本仓库，整理 workspace 规则和质量门槛

#### 仓库动作

- 明确哪些 crate 是示例，哪些 crate 是项目
- 后续新增 crate 前，先定义命名规则和目标说明
- 固定开发基线：
  - `cargo fmt`
  - `cargo clippy --workspace --all-targets --all-features -- -D warnings`
  - `cargo test --workspace`

#### 进入下一阶段的标准

- 你能在 30 分钟内从零建一个小 crate，并配好格式化、lint、测试
- 你能说清 `ownership`、`borrowing`、`Result`、workspace、edition 的基本含义

---

### 阶段 1：Rust 核心深化（4-6 周）

#### 目标

- 从“知道语法”进入“会设计数据和模块”
- 把 borrow checker 从障碍变成设计反馈器

#### 必学主题

- ownership / borrowing / lifetimes
- `Option` / `Result`
- pattern matching
- trait / generic / iterator
- 标准库集合
- 模块边界与 crate 组织
- 测试与文档测试

#### 主文档

- [Rust By Example](https://doc.rust-lang.org/rust-by-example/)
- [The Rust Reference](https://doc.rust-lang.org/reference/)
  - 这不是起步教材，而是遇到语言细节时的权威查询手册
- [The Rust Programming Language](https://doc.rust-lang.org/book/title-page.html)
  - 重点继续读第 8-13 章、第 15-18 章

#### 主仓库

- [rust-lang/rustlings](https://github.com/rust-lang/rustlings)

#### 主书

- [Rust for Rustaceans](https://rust-for-rustaceans.com/)
  - 这是你从“入门者”走向“中级 Rust 使用者”的第一本值得认真读的书

#### 可选补充书

- [Programming Rust, 2nd Edition](https://www.oreilly.com/library/view/programming-rust-2nd/9781492052586/)
  - 适合作为更厚、更系统的中高级参考书

#### 执行方式

1. 不要重新从头学语法，直接按主题推进
2. 每个主题都做一个最小 lab，而不是只写单文件片段
3. 每学完一个主题，必须做一次“概念解释 + 代码验证 + 总结”

#### 推荐主题与本仓库落地

- `ownership_lab`
  - 所有权转移、借用、切片、结构体方法
- `error_lab`
  - `Result`、错误传播、应用层错误 vs 库层错误
- `collections_lab`
  - `Vec`、`HashMap`、`BTreeMap`、迭代器链式处理

#### 本阶段最该读的现有代码

- 先重读你自己的 [crates/todo_cli/src/main.rs](/run/media/zyg/OpenV/Projects/rust/crates/todo_cli/src/main.rs)
- 思考：
  - 哪些逻辑应该抽到 `lib.rs`
  - 哪些错误处理不够清晰
  - 哪些职责耦合太重

#### 进入下一阶段的标准

- 你能独立写一个中等复杂度 CLI
- 你能把主要逻辑放进库层而不是全塞进 `main.rs`
- 遇到借用报错时，大多数情况能通过数据设计解决

---

### 阶段 2：工程化进阶（4-6 周）

#### 目标

- 从“能跑”升级到“可维护、可测试、可扩展”

#### 必学主题

- 错误处理
- CLI 设计
- 序列化与配置
- 日志与可观测性
- 单元测试 / 集成测试 / 样例测试
- crate 对外 API 的组织方式

#### 主文档

- [clap docs](https://docs.rs/clap/latest/clap/)
- [serde docs](https://docs.rs/serde/latest/serde/)
- [tracing-subscriber docs](https://docs.rs/tracing-subscriber)
- [Rust API Guidelines](https://rust-lang.github.io/api-guidelines/checklist.html)
- [cargo-nextest](https://nexte.st/)
- [cargo-deny](https://embarkstudios.github.io/cargo-deny/)

#### 主仓库

- [LukeMathWalker/wiremock-rs](https://github.com/LukeMathWalker/wiremock-rs)
  - 用来学测试友好的 HTTP mock 思路

#### 主书

- [Command-Line Rust](https://www.oreilly.com/library/view/command-line-rust/9781098109424/)

#### 工程库建议

- 应用层错误：`anyhow`
- 库层错误：`thiserror`
- CLI：`clap`
- 日志：`tracing` + `tracing-subscriber`

#### 执行方式

1. 以 `todo_cli` 为主项目做第一次像样重构
2. 目标不是“加很多功能”，而是建立工程习惯
3. 每做一步改动都问自己：
  - 这个逻辑是否可测试
  - 这个错误是否可理解
  - 这个模块是否职责单一

#### `todo_cli` 改造建议

- 从单文件改成 `lib + bin`
- 抽出存储层
- 抽出命令执行层
- 增加测试
- 把输出层和业务层分开
- 允许后续替换 JSON 存储

#### 本阶段重点不是

- 花哨架构
- 复杂 trait 抽象
- 提前设计一堆通用框架

#### 进入下一阶段的标准

- 你能把一个小工具写到“别人能快速看懂、能运行、能改”的水平
- 你已经不再依赖到处散落的 `unwrap` / `expect`

---

### 阶段 3：服务与系统能力（6-8 周）

#### 目标

- 真正进入 Rust 工程主战场
- 学会写一个可运行、可调试、可测试的 API 服务

#### 必学主题

- async/await 心智模型
- `tokio`
- `axum`
- `sqlx`
- 数据库 schema / migration / query
- 共享状态、取消、超时、重试
- 基础性能意识

#### 主文档

- [Tokio tutorial](https://tokio.rs/tokio/tutorial)
- [Async Book](https://rust-lang.github.io/async-book/)
  - 说明：官方书目前处于重写过程中，适合补原理，不适合当唯一入口
- [axum docs](https://docs.rs/crate/axum/latest)
- [sqlx docs](https://docs.rs/crate/sqlx/latest)
- [tracing docs](https://docs.rs/tracing/latest/tracing/)

#### 主仓库

- [tokio-rs/mini-redis](https://github.com/tokio-rs/mini-redis)

#### 第二参考仓库

- [launchbadge/realworld-axum-sqlx](https://github.com/launchbadge/realworld-axum-sqlx)

#### 主书

- [Zero To Production In Rust](https://www.zero2prod.com/assets/sample_zero2prod.pdf)
- 对应代码仓库：[LukeMathWalker/zero-to-production](https://github.com/LukeMathWalker/zero-to-production)

#### 补充书

- [Rust Atomics and Locks](https://mara.nl/atomics/)
  - 这本书在线可读，适合在你开始接触并发和共享状态后插入学习

#### 执行顺序

1. 先完整跑一遍 Tokio tutorial 的核心章节
2. 再读 `mini-redis`，重点学：
   - 任务模型
   - 共享状态
   - 通道
   - graceful shutdown
3. 然后写你自己的第一个 API 服务
4. 再借 `realworld-axum-sqlx` 看项目结构与分层
5. 最后用 `Zero To Production` 补测试、配置、观测、部署思路

#### 本仓库建议新增项目

- `task_api`
  - REST API
  - 数据库存储
  - tracing
  - migration
  - 最小集成测试

#### 进入下一阶段的标准

- 你能独立完成一个 API 服务
- 你知道为什么不能把阻塞操作直接塞进 async 主路径
- 你开始对日志、配置、测试和数据库 schema 有工程意识

---

### 阶段 4：AI 基础闭环（6-8 周）

#### 目标

- 不再把 AI 理解成“调一个模型接口”
- 打通最小 AI 概念链：数据、token、embedding、检索、生成、评测

#### 必学主题

- 向量、相似度、embedding 的最低必要数学
- tokenization
- retrieval vs rerank vs generation
- 数据清洗、切分、索引
- 评测与失败分析

#### 主文档 / 主课程

- [Hugging Face NLP Course](https://huggingface.co/learn/nlp-course)
  - 重点是 tokenizer、datasets、基础 Transformer/LLM 工作流
- [Google Machine Learning Crash Course](https://developers.google.com/machine-learning/crash-course)
  - 用来补最低必要的 ML 概念
- [Dive into Deep Learning](https://d2l.ai/)
  - 如果你觉得自己的 DL 基础明显不足，再选读相关章节

#### Rust 侧主文档

- [Tokenizers docs](https://huggingface.co/docs/tokenizers/main/en/index)
- [tokenizers crate docs](https://docs.rs/crate/tokenizers/latest)
- [safetensors crate docs](https://docs.rs/crate/safetensors/latest)
- [Polars user guide](https://docs.pola.rs/)

#### 主仓库

- `huggingface/tokenizers`
- `huggingface/safetensors`

#### 执行方式

1. 先用课程和文档建立概念，不着急本地推理
2. 先做“检索式 AI”而不是“自己训练模型”
3. 先把数据流做出来：
   - 输入文档
   - 切块
   - tokenizer/embedding
   - 相似度检索
   - 输出结果
   - 简单评测

#### 本仓库建议新增项目

- `embedding_demo`
  - 读取本地文档
  - 切块
  - 生成 embedding
  - 做最小检索

#### 为什么这一阶段先不重压本地模型

- 本地模型推理会引入模型格式、后端、硬件、量化、内存占用等复杂性
- 你现在最需要先学清楚“AI 应用为什么有用、在哪里失真、如何评估”

#### 进入下一阶段的标准

- 你能清楚解释 tokenizer、embedding、检索、生成之间的区别
- 你能做出一个最小 RAG/检索 demo
- 你知道“感觉效果不错”不等于“完成评测”

---

### 阶段 5：Rust AI 实战（6-8 周）

#### 目标

- 把 Rust 与 AI 真正结合起来
- 做出可演示、可扩展的 Rust AI MVP

#### 必学主题

- 模型接入
- 权重格式
- 本地推理 vs 外部 API
- 推理服务化
- 模型缓存、批处理、吞吐与延迟的基本意识

#### 主文档

- [Candle integration docs](https://huggingface.co/docs/transformers/community_integrations/candle)
- [huggingface/candle](https://github.com/huggingface/candle)
- [The Burn Book](https://burn.dev/books/burn/)
- [tracel-ai/burn](https://github.com/tracel-ai/burn)
- [ort crate docs](https://docs.rs/crate/ort/latest)

#### 主仓库

- [huggingface/candle](https://github.com/huggingface/candle)

#### 第二参考仓库

- [tracel-ai/burn](https://github.com/tracel-ai/burn)

#### 如何选技术路线

- 如果你的目标是尽快做产品级 AI 应用：
  - 先用模型 API
  - Rust 负责服务、流程、工具、检索、评测、观测
- 如果你的目标是进入 Rust AI 系统深水区：
  - 逐步学习 Candle / ONNX Runtime / Burn

#### Candle / Burn / ONNX Runtime 的定位

- Candle：
  - 更适合本地推理、模型接入、理解 Rust 侧推理框架
- Burn：
  - 更适合需要训练、模型导入、想系统了解 Rust DL 框架时
- ONNX Runtime Rust：
  - 更适合工程落地、模型复用和稳定推理接入

#### 推荐执行顺序

1. 用外部模型 API 先做通路
2. 把检索、工具调用、评测和服务骨架做稳
3. 再把局部组件替换为本地 embedding 或本地推理
4. 最后再深入 Candle / Burn / ORT 的更底层能力

#### 本仓库建议新增项目

- `rag_service`
- `embedding_worker`
- `model_runner`

#### 进入下一阶段的标准

- 你已经有一个可演示、可讲架构、可继续扩展的 Rust AI 项目
- 你不再把“AI”理解成一个神秘 API，而是一个可工程化系统

---

### 阶段 6：高成长专精（8-12 周）

#### 目标

- 选一条成长上限高的方向做深
- 做出真正有辨识度的作品集项目

#### 可选方向 A：AI 应用后端

适合资源：

- [Zero To Production In Rust](https://www.zero2prod.com/assets/sample_zero2prod.pdf)
- [launchbadge/realworld-axum-sqlx](https://github.com/launchbadge/realworld-axum-sqlx)
- [Tokio tutorial](https://tokio.rs/tokio/tutorial)

适合做的项目：

- 多租户 AI API
- 任务编排与队列
- 鉴权、配额、评测、日志、审计

#### 可选方向 B：推理与性能

适合资源：

- [huggingface/candle](https://github.com/huggingface/candle)
- [ort crate docs](https://docs.rs/crate/ort/latest)
- [Rust Atomics and Locks](https://mara.nl/atomics/)

适合做的项目：

- 本地推理服务
- 模型缓存、批处理、吞吐优化
- 低延迟推理 API

#### 可选方向 C：数据基础设施

适合资源：

- [Polars user guide](https://docs.pola.rs/)
- [Apache DataFusion docs](https://datafusion.apache.org/index.html)

适合做的项目：

- 文档清洗与切分流水线
- embedding ETL
- 检索索引构建工具

#### 选择原则

- 优先选你愿意长期做作品集的方向
- 优先选能和 Rust 优势真正结合的方向
- 不要只追“最热名词”，要追“你能持续积累的系统能力”

## 7. 2026-04 推荐资源总清单

这部分只列我认为值得进入主线的资源，不追求大全。

### Rust 基础与语言

- [The Rust Programming Language](https://doc.rust-lang.org/book/title-page.html)
- [Rust By Example](https://doc.rust-lang.org/rust-by-example/)
- [The Rust Reference](https://doc.rust-lang.org/reference/)
- [The Cargo Book](https://doc.rust-lang.org/cargo/)
- [Rust 2024 Edition Guide](https://doc.rust-lang.org/edition-guide/rust-2024/index.html)
- [rust-lang/rustlings](https://github.com/rust-lang/rustlings)

### Rust 中级到高级

- [Rust for Rustaceans](https://rust-for-rustaceans.com/)
- [Programming Rust, 2nd Edition](https://www.oreilly.com/library/view/programming-rust-2nd/9781492052586/)
- [Rust API Guidelines](https://rust-lang.github.io/api-guidelines/checklist.html)
- [Rust Atomics and Locks](https://mara.nl/atomics/)

### Rust 工程化与 CLI

- [Command-Line Rust](https://www.oreilly.com/library/view/command-line-rust/9781098109424/)
- [clap docs](https://docs.rs/clap/latest/clap/)
- [serde docs](https://docs.rs/serde/latest/serde/)
- [tracing-subscriber docs](https://docs.rs/tracing-subscriber)
- [wiremock docs](https://docs.rs/wiremock/latest/wiremock/)
- [cargo-nextest](https://nexte.st/)
- [cargo-deny](https://embarkstudios.github.io/cargo-deny/)

### Rust 服务端与 async

- [Tokio tutorial](https://tokio.rs/tokio/tutorial)
- [tokio-rs/mini-redis](https://github.com/tokio-rs/mini-redis)
- [Async Book](https://rust-lang.github.io/async-book/)
- [axum docs](https://docs.rs/crate/axum/latest)
- [sqlx docs](https://docs.rs/crate/sqlx/latest)
- [launchbadge/realworld-axum-sqlx](https://github.com/launchbadge/realworld-axum-sqlx)
- [LukeMathWalker/zero-to-production](https://github.com/LukeMathWalker/zero-to-production)
- [Zero To Production In Rust sample](https://www.zero2prod.com/assets/sample_zero2prod.pdf)

### AI 基础与应用

- [Hugging Face NLP Course](https://huggingface.co/learn/nlp-course)
- [Google Machine Learning Crash Course](https://developers.google.com/machine-learning/crash-course)
- [Dive into Deep Learning](https://d2l.ai/)
- [Tokenizers docs](https://huggingface.co/docs/tokenizers/main/en/index)
- [tokenizers crate docs](https://docs.rs/crate/tokenizers/latest)
- [safetensors crate docs](https://docs.rs/crate/safetensors/latest)
- [Polars user guide](https://docs.pola.rs/)
- [Apache DataFusion docs](https://datafusion.apache.org/index.html)

### Rust AI

- [Candle integration docs](https://huggingface.co/docs/transformers/community_integrations/candle)
- [huggingface/candle](https://github.com/huggingface/candle)
- [The Burn Book](https://burn.dev/books/burn/)
- [tracel-ai/burn](https://github.com/tracel-ai/burn)
- [ort crate docs](https://docs.rs/crate/ort/latest)

## 8. 对这个仓库的具体演进建议

这个仓库不需要立刻大改结构，但需要明确演进顺序。

建议按下面顺序扩展：

1. 保留 `playground`，只放最小语法与标准库示例
2. 重构 `todo_cli`，把它升级成第一个像样的小项目
3. 新增 `ownership_lab`、`error_lab`、`collections_lab`
4. 新增一个 async/service 项目，例如 `task_api`
5. 新增一个 AI 项目，例如 `embedding_demo`
6. 最后再做 `rag_service`

不要现在就一次性新增十几个 crate。

正确做法是：

- 每完成一个阶段，再新增下一阶段的 crate
- 每个 crate 必须有明确目标、运行方式、阶段总结

建议的长期结构可以是：

```text
crates/
  playground/
  todo_cli/
  ownership_lab/
  error_lab/
  collections_lab/
  task_api/
  embedding_demo/
  rag_service/
```

## 9. 未来 90 天的最优行动顺序

### 第 1-14 天

- 完成阶段 0
- 读 Rust Book 前半部分和 Cargo Book 关键部分
- 跑一遍 Rustlings
- 固定 fmt / clippy / test 基线
- 给 `todo_cli` 列出重构清单

### 第 15-35 天

- 完成阶段 1
- 新增 `ownership_lab`、`error_lab`、`collections_lab`
- 重点突破 ownership、error handling、collections、traits
- 每个主题至少写 1 个小实验

### 第 36-60 天

- 完成阶段 2
- 以 `todo_cli` 为主项目做一次像样重构
- 做 `lib + bin` 分层
- 加测试
- 加更清楚的错误处理
- 引入 `tracing`

### 第 61-75 天

- 进入阶段 3
- 跑 Tokio tutorial 与 `mini-redis`
- 新建第一个 async/http 服务项目
- 接数据库
- 形成最小 API 服务闭环

### 第 76-90 天

- 开始阶段 4
- 学 tokenizer / embedding / retrieval 基础
- 做一个 `embedding_demo`
- 可以先接模型 API，不强求本地模型
- 但必须把“输入 -> 检索 -> 输出 -> 评测”链条跑通

## 10. 每周节奏怎么安排最稳

推荐节奏：

- 2 次理论输入：每次 1.5-2 小时
- 2 次代码训练：每次 1.5-2 小时
- 1 次长时项目推进：4-5 小时
- 1 次复盘：1 小时

每周必须形成闭环：

`学习 -> 编码 -> 测试 -> 总结`

如果没有总结，知识会碎。

如果没有测试，代码会虚。

如果没有项目，成长会慢。

## 11. 必须避免的坑

- 不要把继续看入门语法例子当成成长
- 不要在 Rust 基础不稳时就沉迷 async 魔法和宏技巧
- 不要 clone 一堆大仓库却不读测试、不读 examples、不做自己的项目
- 不要把 AI 等同于“调一个模型接口”
- 不要把 AI 等同于“必须自己训练模型”
- 不要并行开太多线：Rust 核心、服务端、AI、算法、OS 一起开，会直接失速
- 不要只有 demo，没有测试、文档和复盘

## 12. 最终判断

你这条路线的正确打开方式，不是“先成为刷题选手”，也不是“先成为模型调参选手”。

真正高成长、且适合 Rust 的路径是：

- 用 Rust 建立扎实工程能力
- 用服务端与 async 实践建立复杂项目承载力
- 用 AI 闭环项目把数据、模型、检索、推理、评测串起来
- 最后在 Rust AI 的某个方向做深

这条路线的优势在于：

- 成长上限高
- 成果形成快
- 可以持续沉淀到这个仓库里
- 未来不管你偏应用、偏系统、偏 AI infra，都能接得住

接下来最重要的，不是再找一份更长的书单，而是按阶段持续交付。
