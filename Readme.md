# CSRust

这是一个长期维护的 Rust 学习仓库，现在已经整理成 Cargo workspace，方便同时放置：

- 短小的语法示例
- 跟着教程写的小项目
- 以后持续新增的练习 crate

## 当前结构

```text
CSRust/
  Cargo.toml
  Readme.md
  crates/
    playground/
      Cargo.toml
      src/main.rs
      examples/
        example_01_shadowing.rs
    guessing_game/
      Cargo.toml
      src/main.rs
    todo_cli/
      Cargo.toml
      src/main.rs
      tasks.json
```

## 为什么这样组织

- `playground`：放单知识点示例，适合快速运行和对照学习
- `guessing_game`、`todo_cli`：放相对完整的小项目，各自独立、依赖清晰
- workspace：以后新增 crate 时不需要重建仓库结构，直接在 `crates/` 下扩展即可

## 常用命令

```bash
cargo run -p playground
cargo run -p playground --example example_01_shadowing

cargo run -p guessing_game

cargo run -p todo_cli -- list
cargo run -p todo_cli -- add "学习所有权"
cargo run -p todo_cli -- complete 1
```

## 新增学习内容的建议

- 想加一个简短语法示例：放到 `crates/playground/examples/`
- 想加一个完整练手项目：新建 `crates/<project_name>/`
- 想按专题扩展：可以后续增加 `ownership_lab`、`collections_lab`、`async_lab` 这类 crate

## 目前已经整理好的内容

- `crates/playground/examples/example_01_shadowing.rs`：变量遮蔽
- `crates/guessing_game/src/main.rs`：猜数字游戏
- `crates/todo_cli/src/main.rs`：命令行待办事项
