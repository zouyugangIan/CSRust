# BridgeLab 架构说明

BridgeLab 是面向教学与前期方案验证的二维多跨梁有限元程序。当前版本采用
Euler–Bernoulli 梁理论，只处理线弹性小变形静力问题，不能替代桥梁设计规范验算。

## 分层

```text
bridge-core        领域模型、SI 单位、稳定 ID、模型不变量
      ↓
bridge-solver      网格细分、刚度组装、约束、线性求解、结果恢复
      ↓
bridge-validation  平衡、边界条件和结果有限性检查

bridge-io          版本化 DTO、迁移和原子文件提交
      ↓
bridge-app         GPUI 状态、命令历史、后台任务和可视化
```

`bridge-core` 不依赖 GPUI、Serde 或矩阵库。工程文件 DTO 与领域模型显式转换，避免界面
缓存或内部实现细节进入长期文件格式。

## 状态与计算

- `BridgeModel` 是分析输入的唯一事实来源；所有尺寸字段在名称中标明 SI 单位。
- 节点、单元、材料、截面和荷载使用不同的强类型 ID，不把 `Vec` 下标当持久标识。
- 编辑器把连续键入合并为一个撤销命令，离散操作单独进入最多 100 项的历史。
- 输入改变后等待 80 ms，只接受最新 revision 的后台求解结果；旧任务不能覆盖新模型。
- 画布持有不可变 `Arc<Analysis>` 快照，渲染阶段不组装刚度矩阵。
- 有工程路径时，稳定模型在停止编辑 900 ms 后自动原子保存。
- 保存任务带文档代次和写入代次；切换工程或更新保存请求后，迟到任务不能污染当前状态。

## 工程文件

- 扩展名：`.bridge.json`
- 当前 `schema_version`：1
- v0 单跨梁文件可自动迁移；高于当前版本的文件会被明确拒绝。
- 保存先写同目录临时文件并同步，再原子替换目标文件。写入失败时原文件保持不变。
- 打开/另存为使用 GPUI 原生异步路径提示；读写均强制 16 MiB 上限。
- GPUI Entity、光标、撤销栈、画布缓存和分析结果不会写入工程文件。

## 当前求解范围

- 水平多跨等截面或分单元材料/截面的连续梁。
- 每个分析节点具有竖向位移和转角两个自由度。
- 集中力、集中力矩、均布和线性变化分布荷载的求解层支持。
- GPUI 快速编辑器目前只暴露一个竖向集中力；打开包含其他荷载的文件时会拒绝编辑，
  不会静默删除数据。
- 默认稠密主元消去后端适合小模型；`LinearSystemSolver` 接口是未来稀疏后端的边界。

## 下一阶段

1. 荷载表格、多个荷载工况与组合。
2. 移动荷载、影响线及内力包络。
3. 二维框架单元（`ux / uy / rz`）和支座沉降。
4. 稀疏矩阵、条件数/机构诊断和模型规模基准。
5. 温度、预应力及经过独立验证的规范组合。

不应在完成解析解、商业软件对照、网格收敛和独立审查前加入“工程设计可用”声明。

## 运行与质量门

```bash
cargo run -p bridge-app
cargo test -p bridge-core -p bridge-solver -p bridge-io -p bridge-validation -p bridge-app --all-targets
cargo clippy -p bridge-core -p bridge-solver -p bridge-io -p bridge-validation -p bridge-app --all-targets -- -D warnings
rustfmt --edition 2024 --check \
  crates/bridge_core/src/lib.rs crates/bridge_solver/src/lib.rs \
  crates/bridge_io/src/lib.rs crates/bridge_validation/src/lib.rs \
  crates/bridge_app/src/lib.rs crates/bridge_app/src/main.rs
```

兼容入口仍可使用：

```bash
cargo run -p playground --example 24_forceGraph
```
