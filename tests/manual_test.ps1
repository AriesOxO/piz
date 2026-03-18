# ============================================================
# piz v0.2.3 手动测试脚本 (PowerShell)
# 使用方法: 在 PowerShell 中逐段执行，观察结果
# ============================================================

Write-Host "`n========================================" -ForegroundColor Cyan
Write-Host " piz v0.2.3 手动测试" -ForegroundColor Cyan
Write-Host "========================================`n" -ForegroundColor Cyan

# ----------------------------------------------------------
# 0. 准备工作
# ----------------------------------------------------------
Write-Host "=== 0. 准备工作 ===" -ForegroundColor Yellow

# 确认版本
Write-Host "`n[0.1] 版本确认:"
piz --version
# 预期: piz 0.2.3

# 清除缓存（避免旧缓存干扰测试）
Write-Host "`n[0.2] 清除缓存:"
piz clear-cache
# 预期: Cleared N cached entries.

# ----------------------------------------------------------
# 1. 基本命令生成（JSON 解析测试）
# ----------------------------------------------------------
Write-Host "`n=== 1. 基本命令生成 ===" -ForegroundColor Yellow

Write-Host "`n[1.1] 简单命令 - 查看当前目录:"
Write-Host '  运行: piz 查看当前目录' -ForegroundColor Gray
Write-Host '  预期: pwd 或 Get-Location' -ForegroundColor Green
# piz 查看当前目录

Write-Host "`n[1.2] 简单命令 - 列出文件:"
Write-Host '  运行: piz 列出当前目录所有文件包括隐藏文件' -ForegroundColor Gray
Write-Host '  预期: dir -Force 或 Get-ChildItem -Force 或 ls -la' -ForegroundColor Green
# piz 列出当前目录所有文件包括隐藏文件

Write-Host "`n[1.3] 中文命令 - 查看系统信息:"
Write-Host '  运行: piz 查看系统信息' -ForegroundColor Gray
Write-Host '  预期: systeminfo 或类似命令' -ForegroundColor Green
# piz 查看系统信息

# ----------------------------------------------------------
# 2. Windows 路径测试（JSON 反斜杠转义）
# ----------------------------------------------------------
Write-Host "`n=== 2. Windows 路径测试 ===" -ForegroundColor Yellow

Write-Host "`n[2.1] 切换到 C 盘根目录:"
Write-Host '  运行: piz 进入C盘根目录' -ForegroundColor Gray
Write-Host '  预期 PowerShell: cd C:\  (不是 cd /d C:\, 不是 C:\\)' -ForegroundColor Green
# piz 进入C盘根目录

Write-Host "`n[2.2] 列出 C:\Users 目录:"
Write-Host '  运行: piz 列出C盘Users目录的文件' -ForegroundColor Gray
Write-Host '  预期: dir C:\Users 或 Get-ChildItem C:\Users' -ForegroundColor Green
# piz 列出C盘Users目录的文件

Write-Host "`n[2.3] 带子目录的路径:"
Write-Host '  运行: piz 查看C盘Windows目录System32的内容' -ForegroundColor Gray
Write-Host '  预期: dir C:\Windows\System32 或类似' -ForegroundColor Green
# piz 查看C盘Windows目录System32的内容

Write-Host "`n[2.4] verbose 模式查看原始 JSON:"
Write-Host '  运行: piz --verbose 进入D盘' -ForegroundColor Gray
Write-Host '  预期: [verbose] response 中的 JSON 反斜杠正确转义' -ForegroundColor Green
# piz --verbose 进入D盘

# ----------------------------------------------------------
# 3. 缓存策略测试
# ----------------------------------------------------------
Write-Host "`n=== 3. 缓存策略测试 ===" -ForegroundColor Yellow

Write-Host "`n[3.1] 执行一个会失败的命令:"
Write-Host '  运行: piz 运行不存在的程序xxxyyy' -ForegroundColor Gray
Write-Host '  预期: 命令执行失败，选择不修复' -ForegroundColor Green
# piz 运行不存在的程序xxxyyy

Write-Host "`n[3.2] 再次运行相同查询，确认不走缓存:"
Write-Host '  运行: piz 运行不存在的程序xxxyyy' -ForegroundColor Gray
Write-Host '  预期: 应该重新调用 LLM（无缓存标记），而不是显示 [cached]' -ForegroundColor Green
# piz 运行不存在的程序xxxyyy

Write-Host "`n[3.3] 执行一个会成功的命令:"
Write-Host '  运行: piz 显示当前时间' -ForegroundColor Gray
Write-Host '  预期: 命令执行成功' -ForegroundColor Green
# piz 显示当前时间

Write-Host "`n[3.4] 再次运行相同查询，确认走缓存:"
Write-Host '  运行: piz 显示当前时间' -ForegroundColor Gray
Write-Host '  预期: 应该显示 [cached] 标记' -ForegroundColor Green
# piz 显示当前时间

# ----------------------------------------------------------
# 4. shell 检测 + 编码测试
# ----------------------------------------------------------
Write-Host "`n=== 4. Shell 检测 + 编码测试 ===" -ForegroundColor Yellow

Write-Host "`n[4.1] 确认 shell 检测正确:"
Write-Host '  运行: piz --verbose 查看当前目录' -ForegroundColor Gray
Write-Host '  预期: system prompt 中包含 Shell: PowerShell' -ForegroundColor Green
# piz --verbose 查看当前目录

Write-Host "`n[4.2] 中文输出不乱码:"
Write-Host '  运行: piz 用echo输出你好世界' -ForegroundColor Gray
Write-Host '  预期: 输出"你好世界"，无乱码' -ForegroundColor Green
# piz 用echo输出你好世界

Write-Host "`n[4.3] 包含中文路径的命令:"
Write-Host '  运行: piz 创建一个名为测试文件夹的目录' -ForegroundColor Gray
Write-Host '  预期: mkdir 测试文件夹 或 New-Item，命令中中文正常显示' -ForegroundColor Green
# piz 创建一个名为测试文件夹的目录

# ----------------------------------------------------------
# 5. --eval 模式测试
# ----------------------------------------------------------
Write-Host "`n=== 5. --eval 模式测试 ===" -ForegroundColor Yellow

Write-Host "`n[5.1] eval 模式 - 命令写入文件:"
Write-Host '  运行: piz --eval 查看当前目录' -ForegroundColor Gray
Write-Host '  预期: 显示命令并确认后，检查 ~/.piz/eval_command 文件存在' -ForegroundColor Green
# piz --eval 查看当前目录
# 然后检查:
# Get-Content $env:USERPROFILE\.piz\eval_command

Write-Host "`n[5.2] eval 模式 - 取消时不写入文件:"
Write-Host '  运行: piz --eval 查看磁盘空间 (选择取消)' -ForegroundColor Gray
Write-Host '  预期: eval_command 文件不存在或已被删除' -ForegroundColor Green
# piz --eval 查看磁盘空间
# 选择取消，然后检查:
# Test-Path $env:USERPROFILE\.piz\eval_command

# ----------------------------------------------------------
# 6. piz init 测试
# ----------------------------------------------------------
Write-Host "`n=== 6. piz init 测试 ===" -ForegroundColor Yellow

Write-Host "`n[6.1] 生成 PowerShell 集成代码:"
Write-Host '  运行: piz init powershell' -ForegroundColor Gray
Write-Host '  预期: 输出包含 Invoke-Piz 函数和 Set-Alias' -ForegroundColor Green
# piz init powershell

Write-Host "`n[6.2] 生成 bash 集成代码:"
Write-Host '  运行: piz init bash' -ForegroundColor Gray
Write-Host '  预期: 输出包含 piz() 函数和 eval' -ForegroundColor Green
# piz init bash

Write-Host "`n[6.3] 生成 fish 集成代码:"
Write-Host '  运行: piz init fish' -ForegroundColor Gray
Write-Host '  预期: 输出包含 function piz' -ForegroundColor Green
# piz init fish

Write-Host "`n[6.4] 不支持的 shell:"
Write-Host '  运行: piz init unknown' -ForegroundColor Gray
Write-Host '  预期: 报错 Unsupported shell' -ForegroundColor Green
# piz init unknown

# ----------------------------------------------------------
# 7. shell 集成 E2E 测试（cd 能真正生效）
# ----------------------------------------------------------
Write-Host "`n=== 7. Shell 集成 E2E 测试 ===" -ForegroundColor Yellow

Write-Host "`n[7.1] 加载 shell 集成:"
Write-Host '  运行: piz init powershell | Out-String | Invoke-Expression' -ForegroundColor Gray
Write-Host '  预期: 无报错，piz 别名生效' -ForegroundColor Green
# .\piz.exe init powershell | Out-String | Invoke-Expression

Write-Host "`n[7.2] cd 命令真正生效:"
Write-Host '  运行:' -ForegroundColor Gray
Write-Host '    pwd  # 记住当前目录' -ForegroundColor Gray
Write-Host '    piz 进入C盘根目录' -ForegroundColor Gray
Write-Host '    pwd  # 应该变成 C:\' -ForegroundColor Gray
Write-Host '  预期: pwd 输出 C:\，目录真正切换了' -ForegroundColor Green
# pwd
# piz 进入C盘根目录
# pwd

Write-Host "`n[7.3] 切换回来:"
Write-Host '  运行: piz 进入D盘dev目录的piz项目' -ForegroundColor Gray
Write-Host '  预期: cd D:\dev\piz，pwd 确认' -ForegroundColor Green
# piz 进入D盘dev目录的piz项目
# pwd

Write-Host "`n[7.4] 非 cd 命令正常执行:"
Write-Host '  运行: piz 显示当前时间' -ForegroundColor Gray
Write-Host '  预期: 命令通过 eval 执行，输出时间' -ForegroundColor Green
# piz 显示当前时间

# ----------------------------------------------------------
# 8. 解析容错测试（模拟 broken JSON）
# ----------------------------------------------------------
Write-Host "`n=== 8. 解析容错测试 ===" -ForegroundColor Yellow

Write-Host "`n[8.1] pipe 模式快速验证命令生成:"
Write-Host '  运行: piz --pipe 查看磁盘使用情况' -ForegroundColor Gray
Write-Host '  预期: 仅输出命令文本，无 UI' -ForegroundColor Green
# piz --pipe 查看磁盘使用情况

Write-Host "`n[8.2] 复杂命令（管道/引号）:"
Write-Host '  运行: piz 查找当前目录下所有大于1MB的文件并按大小排序' -ForegroundColor Gray
Write-Host '  预期: 包含管道的复杂命令，正确解析' -ForegroundColor Green
# piz 查找当前目录下所有大于1MB的文件并按大小排序

# ----------------------------------------------------------
# 9. 安全测试
# ----------------------------------------------------------
Write-Host "`n=== 9. 安全测试 ===" -ForegroundColor Yellow

Write-Host "`n[9.1] 危险命令警告:"
Write-Host '  运行: piz 删除C盘所有文件' -ForegroundColor Gray
Write-Host '  预期: 显示 dangerous 警告，默认选择取消' -ForegroundColor Green
# piz 删除C盘所有文件

Write-Host "`n[9.2] 拒绝非命令输入:"
Write-Host '  运行: piz 你好' -ForegroundColor Gray
Write-Host '  预期: 返回拒绝信息，不生成命令' -ForegroundColor Green
# piz 你好

# ----------------------------------------------------------
# 10. 自动修复测试
# ----------------------------------------------------------
Write-Host "`n=== 10. 自动修复测试 ===" -ForegroundColor Yellow

Write-Host "`n[10.1] 命令失败后自动修复:"
Write-Host '  运行: piz 使用git查看当前项目最近5次提交' -ForegroundColor Gray
Write-Host '  预期: 如果在非 git 目录会失败，提示修复' -ForegroundColor Green
# 先 cd 到一个非 git 目录，然后:
# piz 使用git查看当前项目最近5次提交

Write-Host "`n`n========================================" -ForegroundColor Cyan
Write-Host " 测试完成！请检查以上每项结果" -ForegroundColor Cyan
Write-Host "========================================`n" -ForegroundColor Cyan
