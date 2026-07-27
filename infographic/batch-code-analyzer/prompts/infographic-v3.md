---
title: "Batch Code Analyzer：把批量代码分析变成一个可追溯的本地工作流"
layout: "dense-modules"
style: "pop-laboratory"
aspect_ratio: "9:16"
language: "zh"
use_case: "infographic-diagram"
revision: "v3 reduced text for accuracy"
---

Use case: infographic-diagram
Asset type: 公众号文章竖版信息图，手机端可读的技术产品导览图
Primary request: 生成一张专业、信息密度高但清晰有序的中文技术信息图，介绍本地桌面工具 Batch Code Analyzer。画面组织成 7 个模块：产品定位、七步闭环、项目上下文、四层架构、可靠性、安全边界、三平台发布。只展示给定文字，不补充示例数据。
Scene/backdrop: 淡灰白纸张与细蓝图网格；四周只有无文字的坐标刻度和十字定位线；不生成日期、条码、编号、时间或状态示例。
Subject: 中央是一条从本地代码仓库到 Markdown 结果的流程线，周围是 A-01 到 G-07 七个模块。
Style/medium: pop-laboratory；低饱和青绿色功能块、荧光粉警示、荧光黄数据高亮、炭黑线稿；工程蓝图、技术手册、严格几何模块；不要可爱卡通、照片、渐变、彩虹色、营销装饰。
Composition/framing: 竖版 9:16；顶部主标题；中部七步流程；下部架构、可靠性、安全和发布矩阵；所有文字清晰、无裁切、无重叠。
Lighting/mood: 平面印刷光感，专业、可信、克制。
Color palette: 背景 #F2F2F2；功能区 #B8D8BE；警示 #E91E63；高亮 #FFF200；线稿 #2D2926。
Text (verbatim, and no other text):
主标题：“Batch Code Analyzer”
副标题：“把批量代码分析变成一个可追溯的本地工作流”
模块标题：“A-01 产品定位”“B-02 七步闭环”“C-03 项目上下文”“D-04 四层架构”“E-05 可靠性”“F-06 安全边界”“G-07 三平台发布”
产品定位标签：“本地桌面工具”“批量代码分析”“可追溯的本地工作流”“不修改源代码”“请求数上限 1—30”“默认 256 KB”“10,000 个文件”
流程标签：“扫描”“提示词”“上下文摘要”“创建 Run”“执行请求”“Markdown 结果”“闭环完成”
上下文标签：“README”“AGENTS.md”“项目上下文摘要”“文件职责”“数据流”
架构标签：“React UI”“Rust 核心”“SQLite”“Secret Store”“Provider Adapter”
可靠性标签：“Run”“Task”“Attempt”“新增记录”“不覆盖历史”“不自动重发结果未知请求”
安全标签：“密钥隔离”“路径校验”“符号链接”“不修改源代码”
发布标签：“Windows”“macOS”“Linux”“.msi / .exe”“.dmg”“.AppImage / .deb”
Constraints: 严格只使用 Text 中的文字；必须把“请求数上限 1—30”完整、逐字渲染，绝不能出现“开发 1—30”；不能生成 Run-001、时间戳、成功/运行中状态、自动更新、团队协作等示例；不能生成任何伪文字、日期、条码或额外口号；中文正文保持大字号，适合手机端阅读。
Avoid: 错别字、乱码、额外数字、示例时间、示例状态、虚构功能、未定义元数据、人物、照片、源码、API Key、二维码、水印、渐变、紫色主色、橙棕色主色。

Layout guidelines: dense-modules，7 个高密度模块，每个模块有清晰边框、编号和具体标签；中心流程线连接输入仓库和 Markdown 输出；架构模块使用 React UI → Rust 核心 → SQLite / Secret Store / Provider Adapter 的分层结构。

Style guidelines: pop-laboratory，蓝图网格、坐标标记、荧光粉警示、高亮黄数据、青绿色功能块、炭黑工程线稿；严格克制，不添加装饰性文字。

Generate the infographic in portrait 9:16 format for a Chinese WeChat article. Prioritize text accuracy, legibility, and faithful content over decorative density.
