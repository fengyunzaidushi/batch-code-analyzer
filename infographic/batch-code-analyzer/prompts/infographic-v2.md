---
title: "Batch Code Analyzer：把批量代码分析变成一个可追溯的本地工作流"
layout: "dense-modules"
style: "pop-laboratory"
aspect_ratio: "9:16"
language: "zh"
use_case: "infographic-diagram"
revision: "v2 text accuracy"
---

Use case: infographic-diagram
Asset type: 公众号文章竖版信息图，手机端可读的技术产品导览图
Primary request: 生成一张专业、信息密度高但清晰有序的中文技术信息图，介绍本地桌面工具 Batch Code Analyzer。画面必须把产品定位、七步工作流、项目上下文、分层架构、可靠性、安全边界和三平台发布组织成 7 个模块。不要画真实人物，不要使用照片，不要出现品牌 Logo 或水印。
Scene/backdrop: 淡灰白专业纸张底色，细密但克制的蓝图网格，四周只有无文字的坐标刻度、十字定位线和空白技术标记；严禁生成日期、编号、条码或其他未在 Text 中列出的元数据。
Subject: 中央是一条从本地代码仓库到 Markdown 结果的工程流程线，周围环绕 7 个编号模块：A-01 产品定位，B-02 七步闭环，C-03 项目上下文，D-04 四层架构，E-05 可靠性，F-06 安全边界，G-07 三平台发布。
Style/medium: pop-laboratory；严格使用低饱和青绿色、荧光粉、荧光黄、炭黑和灰白；粗重的标题字与纤细的工程标注形成对比；模块边界清楚，技术线稿、箭头、坐标标记和高亮贴纸感并存；不要可爱卡通，不要渐变，不要彩虹色，不要空泛装饰。
Composition/framing: 竖版 9:16；最顶部放主标题和一句副标题；下面采用 2 列不规则但对齐的高密度模块网格；中部让七步流程形成主视觉；架构模块用正确的分层框图：React UI → Rust 核心 → SQLite / Secret Store / Provider Adapter；可靠性模块用 Run → Task → Attempt 时间链；底部用 Windows、macOS、Linux 三个平台矩阵收尾；保留足够内边距，所有文字不得被裁切或重叠。
Lighting/mood: 平面印刷光感，专业、可信、克制、略带实验室手册的紧张感；无阴影堆叠、无立体玻璃拟态。
Color palette: 背景 #F2F2F2；功能区 #B8D8BE；警示与关键结果 #E91E63；高亮 #FFF200；线稿 #2D2926；少量蓝色只作为流程箭头和技术节点。
Materials/textures: 细蓝图网格、印刷套色轻微错位、纸张微纹理、坐标标记和标尺；不要带有文字的条码或日期。
Text (verbatim):
主标题：“Batch Code Analyzer”
副标题：“把批量代码分析变成一个可追溯的本地工作流”
模块标题：“A-01 产品定位”“B-02 七步闭环”“C-03 项目上下文”“D-04 四层架构”“E-05 可靠性”“F-06 安全边界”“G-07 三平台发布”
关键标签：“扫描”“提示词”“上下文摘要”“创建 Run”“执行请求”“Markdown 结果”“Run → Task → Attempt”“密钥隔离”“路径校验”“Windows”“macOS”“Linux”“Mock Provider”“SQLite”“Rust 核心”“React UI”
关键数据：“并发 1～30”“默认 256 KB”“10,000 个文件”“不修改源代码”“不自动重发结果未知请求”
Constraints: 只允许出现上述文字、数字和技术词；必须把“并发 1～30”逐字渲染为“并发 1～30”，绝不能写成“开发 1～30”；必须把“C-03 项目上下文”逐字渲染，不要写成“输入侧信息”；不要添加日期、文档编号、条码或伪文字；标题比正文大，正文至少适合手机端阅读；每个模块都有坐标标记；流程箭头方向明确；信息图必须是完整单张位图。
Avoid: 错别字、乱码、拉丁占位文字、不可读的小字、重复模块、互相覆盖的文本、过度拥挤、营销口号、人物、照片、真实代码源码、API Key、二维码、水印、日期、条码、未定义编号、渐变、紫色主色、橙棕色主色、空白大块背景。

Layout guidelines: 采用 dense-modules 的 7 个高密度信息模块；每个模块有清晰边框、编号和具体数据；模块之间用细箭头连接；主标题置顶；流程与架构占据视觉中心；底部为平台发布矩阵。

Style guidelines: 采用 pop-laboratory 的坐标系统、工程标注、蓝图网格、荧光粉警示、高亮黄标记和青绿色功能块；使用炭黑细线和精确几何图形；不要手绘抖动、不要软萌插画、不要彩色渐变。

Generate the infographic in portrait 9:16 format, optimized for a Chinese WeChat article cover/inline image. Prioritize accurate, legible Chinese typography and strong hierarchy over decorative detail.
