# 领域文档

技能在探索本仓库代码时应如何消费领域文档。

## 探索前先读

- 仓库根目录的 **`GLOSSARY.md`**，或有：
- 仓库根目录的 **`GLOSSARY-MAP.md`**（若存在）：指向每个上下文各自的 `GLOSSARY.md`，读取与主题相关的每一份。
- **`docs/adr/`**：读取将触及的领域相关 ADR。多上下文中还须检查 `src/<context>/docs/adr/` 的上下文级决策。

若上述文件不存在，**静默继续**。不要标记其缺失，也不要主动建议创建。`/domain-modeling` 技能（经由 `/grill-with-docs` 与 `/improve-codebase-architecture` 触达）会在词条或决策实际成形时惰性创建它们。

## 文件结构

单上下文仓库（大多数仓库）：

```
/
├── GLOSSARY.md
├── docs/adr/
│   ├── 0001-event-sourced-orders.md
│   └── 0002-postgres-for-write-model.md
└── src/
```

多上下文仓库（根目录存在 `GLOSSARY-MAP.md`）：

```
/
├── GLOSSARY-MAP.md
├── docs/adr/                          ← 系统级决策
└── src/
    ├── ordering/
    │   ├── GLOSSARY.md
    │   └── docs/adr/                  ← 上下文级决策
    └── billing/
        ├── GLOSSARY.md
        └── docs/adr/
```

## 使用词表的术语

当输出命名某个领域概念（issue 标题、重构提案、假设、测试名）时，使用 `GLOSSARY.md` 中定义的术语，不要漂移到词表明确回避的同义词。

若所需概念尚未出现在词表中，这是一个信号：要么你在凭空发明项目未使用的语言（重新考虑），要么确有空白（记入 `/domain-modeling` 待办）。

## 标注 ADR 冲突

若输出与现有 ADR 相矛盾，须显式标出而非默默覆盖：

> _与 ADR-0007（event-sourced orders）冲突，但因……值得重新讨论_
