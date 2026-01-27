# DroidRun 无障碍树过滤与呈现链路分析

本文基于 `/Users/gokwok/code/droidrun` 代码审视 DroidRun 在 **无障碍树过滤、合并（归并/保留策略）、格式化并呈现给智能体** 的完整链路，重点聚焦 Android 端（Portal + ADB/MobileRun）数据通路。

---

## 1. 数据来源与总流程

**数据来源**  
Android 侧的无障碍树来自 Portal 服务的 `get_state()`（同时包含 `phone_state`、`device_context`）。

**总流程（Android ADB 与 Cloud 模式一致）**  
1) Portal 返回 `a11y_tree` + `phone_state` + `device_context`  
2) 过滤器（`TreeFilter`）对 `a11y_tree` 做裁剪/过滤  
3) 格式化器（`TreeFormatter`）把过滤后的树转换成 **索引列表（扁平化）**  
4) 结果存入缓存（`clickable_elements_cache`），并输出给智能体  

入口位置：
- `droidrun/tools/android/adb.py::get_state()`  
- `droidrun/tools/cloud/cloud.py::get_state()`

---

## 2. 过滤策略（Filter）

过滤逻辑集中在 `droidrun/tools/filters/*`。

### 2.1 过滤器选择
默认选择逻辑在：
- `tools/android/adb.py` 与 `tools/cloud/cloud.py` 的初始化  

规则：
- `vision_enabled=True` → `ConciseFilter`
- `vision_enabled=False` → `DetailedFilter`

### 2.2 ConciseFilter（简洁过滤）
文件：`tools/filters/concise_filter.py`

核心规则：
- **屏幕相交过滤**：只保留与屏幕有交集的节点  
  - 通过 `boundsInScreen` 与屏幕大小判断
- **最小尺寸过滤**：宽高必须大于 `min_element_size`（默认 5px）
- **先判父再遍历**：如果父节点不通过，**子节点直接被丢弃**

> 这意味着 ConciseFilter 是“强剪枝”：父节点失败会连同子树全部移除。

### 2.3 DetailedFilter（详细过滤）
文件：`tools/filters/detailed_filter.py`

可配置项：
- `visibility_threshold`（默认 0.1）  
- `filter_keyboard=True`（默认过滤 Google Keyboard）  
- `clip_bounds=False`（可选裁剪到屏幕范围）

核心规则：
1) **可选裁剪 bounds**：把元素边界裁剪到屏幕（`clip_bounds`）
2) **键盘过滤**：过滤 `resourceId` 前缀为  
   `com.google.android.inputmethod.latin:id/` 的节点
3) **可见面积阈值过滤**：
   - 计算元素可见面积 / 总面积
   - `< visibility_threshold` 的节点 **可能被移除**
   - 但若该节点仍有可见子节点，则 **保留父节点**

> DetailedFilter 的“合并/保留”体现在：父节点不可见但有子节点时会保留父节点，用于保持路径和层级连续性。

此外，节点属性 `ignoreBoundsFiltering == "true"` 会跳过可见性过滤。

---

## 3. “合并 / 归并”行为分析

DroidRun 并没有显式“合并节点”算法，但存在以下 **归并/合并语义**：

1) **父节点保留（DetailedFilter）**  
   通过可见性过滤时，若子节点仍存在，父节点会被保留，用于维持树结构。  
   这相当于“结构性合并”（保留父节点以承接子树）。

2) **字段合并（Formatter）**  
   在输出给智能体的节点中，`text` 字段会按优先级合并：
   ```
   text = text or contentDescription or resourceId or className
   ```
   这属于 **属性级合并**，用于减少空文本节点。

3) **坐标合并（Formatter）**  
   bounds 使用 `boundsInScreen` → `left,top,right,bottom`  
   若启用 `use_normalized`，则使用屏幕宽高归一化。

> 总体而言，DroidRun 的“合并”以 **保留路径 + 属性归并** 为主，而非节点去重或结构融合。

---

## 4. 格式化与“呈现给智能体”（仅无障碍树）

格式化器：`tools/formatters/indexed_formatter.py`

### 4.1 扁平化与索引
过滤后的树会做 **先序遍历**，并分配递增索引：
```python
_flatten_with_index(node, counter=[1])
```

输出 `a11y_tree` 是 **列表形式**（而非原始树），每个元素结构简化为：
```json
{
  "index": 1,
  "resourceId": "...",
  "className": "Button",
  "text": "OK",
  "bounds": "x1,y1,x2,y2",
  "children": []
}
```

> 注意：格式化后的 `children` 永远是空数组，因此原始层级被“扁平化”。

### 4.2 缓存与动作执行
`a11y_tree`（扁平列表）被缓存为 `clickable_elements_cache`，用于后续 **按 index 点击**：
- `tap_by_index`
- `tap_on_index`（避开遮挡）

---

## 5. 总结：DroidRun 的“过滤 → 合并 → 呈现”特征

**过滤**
- Concise：强剪枝，父失败即全剪
- Detailed：可见性阈值 + 保留父节点

**合并**
- 无显式去重/融合  
- 主要体现在：
  - 父节点保留（结构性合并）
  - `text` 字段多属性合并

**呈现**
- 输出给智能体的是 **扁平化 + 索引列表**
- 结构信息不会直接呈现（除非用原始树）

---

## 6. 关键代码索引

- 过滤器选择：  
  `droidrun/tools/android/adb.py`、`droidrun/tools/cloud/cloud.py`
- ConciseFilter：  
  `droidrun/tools/filters/concise_filter.py`
- DetailedFilter：  
  `droidrun/tools/filters/detailed_filter.py`
- 格式化与索引：  
  `droidrun/tools/formatters/indexed_formatter.py`
- 智能体获取状态入口：  
  `droidrun/agent/manager/manager_agent.py::prepare_context`

---

如果需要，我可以进一步对比 DroidRun 与当前 HarmonyOS 版的无障碍树格式，给出 **字段对齐/裁剪/合并策略建议**。
