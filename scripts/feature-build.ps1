#Requires -Version 5.1
<#
.SYNOPSIS
    统一构建入口：固定 UTF-8 控制台与中文代码页，集中注入编译环境变量，按参数裁剪特性。

.DESCRIPTION
    编码
        chcp 65001 + Console.OutputEncoding + $OutputEncoding 三处同时设 UTF-8，
        避免 PowerShell / npm / cargo 混排输出时中文乱码。

    环境变量
        特性清单读仓库级 config/features.json（手写、唯一真源），每项声明
        default / cargoFeature / env；脚本按 env 注入 <ENV>=1 或 0
        （vault → VAULT_ENABLED，与 vite.config.ts 的 define 完全一致）。
        vite.config.ts 也直接读同一份清单，因此前端 vite define 与后端 cargo feature
        在同一次进程调用里取到同一份开关，不存在「前端开 / 后端关」的混合态。
        启动时校验清单与 src-tauri/Cargo.toml 的 [features] 段：cargoFeature 必须
        存在、默认启用集合必须一致、两侧不得有孤儿条目，不一致即中止。

    cargo 配置的加载方式
        cargo 按「当前工作目录 → 各级父目录」查找 .cargo/config.toml，与 --manifest-path
        无关。因此本脚本在 src-tauri/ 下执行 cargo，同时拿到：
            src-tauri/.cargo/config.toml   OPENSSL_DIR / OPENSSL_STATIC、镜像源
            <仓库根>/.cargo/config.toml    TS_RS_EXPORT_DIR / TS_RS_LARGE_INT
        脚本不解析、不覆盖这两个文件，仅按 cargo 自身的规则让它们生效——
        镜像源、[env]、[build]、[target] 等全部配置项都保持项目原样。
        tauri dev/build 在仓库根执行（npm 需要 package.json），
        tauri CLI 自己会以 src-tauri/ 为 cwd 调用 cargo，配置同样完整加载。

    无参数交互
        完全不带参数运行时进入两步交互：先选任务，再选特性。
        选特性时回车 = 沿用 Cargo.toml 的 default，'-' = 全部关闭，逗号分隔 = 覆盖。
        只要带了任意参数（哪怕只带 -Without），就按命令行语义直接执行，不再询问。

    终端标题
        运行期把窗口标题改成 `assistant · <任务> · <特性>`（如 `assistant · 编译检查 · vault`），
        多开终端时一眼区分哪个在编什么；交互阶段显示 `assistant · 选择任务…`。
        脚本结束（含失败退出）后恢复调用前的原标题。

    参数
        -Task       选择动作：dev / build / check / test / lint / typecheck / clean
        -Features   显式启用集合（逗号分隔）；省略时取清单里的 default
        -Without    从启用集合中排除（逗号分隔）
        -NoBundle   build 时跳过打包，只出可执行文件
        -Release    check / test 使用 release profile
        -Target     指定 rust target triple
        -ExtraArgs  透传给 tauri / npm 子命令（位置参数，可省略 -ExtraArgs 直接写）
        -CargoArgs  透传给 cargo runner（tauri 下会落在 `--` 之后）

    参数分层（这是 tauri 特有的三段式，不能混写）
        npm run tauri -- <子命令> <tauri 选项> -- <cargo runner 选项>
        tauri 自己的 `--features` 只负责「开哪些特性」；
        `--no-default-features` 不是 tauri 选项，必须放在 `--` 之后交给 cargo，
        否则 tauri 会报 unexpected argument。脚本自动完成这层嵌套。

.EXAMPLE
    .\scripts\feature-build.ps1
    无参数 → 交互选择任务与特性。

.EXAMPLE
    .\scripts\feature-build.ps1 -Task build
    默认全量（含 vault）打包，实际展开为
    npm run tauri -- build --features vault -- --no-default-features。

.EXAMPLE
    .\scripts\feature-build.ps1 -Task build -Without vault
    无 vault 构建：VAULT_ENABLED=0 + `tauri build -- --no-default-features`。

.EXAMPLE
    .\scripts\feature-build.ps1 -Task dev -Without vault
    无 vault 开发模式，等价于
    $env:VAULT_ENABLED="0"; npm run tauri dev -- -- --no-default-features。

.EXAMPLE
    .\scripts\feature-build.ps1 -Task check -Without vault
    无 vault 的 cargo check（直接调 cargo，不经 tauri，最快验证条件编译）。
#>
[CmdletBinding()]
param(
    [ValidateSet('dev', 'build', 'check', 'test', 'lint', 'typecheck', 'clean')]
    [string]$Task = 'dev',

    [string]$Features,

    [string]$Without,

    [switch]$NoBundle,

    [switch]$Release,

    [string]$Target,

    [Parameter(Position = 0, ValueFromRemainingArguments = $true)]
    [string[]]$ExtraArgs,

    [string[]]$CargoArgs
)

$ErrorActionPreference = 'Stop'

$RepoRoot = Split-Path -Parent $PSScriptRoot
$SrcTauri = Join-Path $RepoRoot 'src-tauri'
$ManifestPath = Join-Path $SrcTauri 'Cargo.toml'
$FeatureManifestPath = Join-Path $RepoRoot 'config/features.json'
$CargoConfigPath = Join-Path $SrcTauri '.cargo/config.toml'
$CargoConfigExamplePath = Join-Path $SrcTauri '.cargo/config.toml.example'

$TaskChoices = @(
    [pscustomobject]@{ Name = 'dev'; Title = '开发'; Desc = '开发模式（前端热更新 + 后端自动重编）'; NeedsRust = $true }
    [pscustomobject]@{ Name = 'build'; Title = '打包'; Desc = '打包安装器'; NeedsRust = $true }
    [pscustomobject]@{ Name = 'check'; Title = '编译检查'; Desc = '只做 cargo 条件编译检查（最快）'; NeedsRust = $true }
    [pscustomobject]@{ Name = 'test'; Title = '后端测试'; Desc = '后端 cargo 测试'; NeedsRust = $true }
    [pscustomobject]@{ Name = 'lint'; Title = '前端检查'; Desc = '前端 eslint'; NeedsRust = $false }
    [pscustomobject]@{ Name = 'typecheck'; Title = '类型检查'; Desc = '前端 tsc --noEmit'; NeedsRust = $false }
    [pscustomobject]@{ Name = 'clean'; Title = '清理'; Desc = 'cargo clean'; NeedsRust = $true }
)

$script:OriginalTitle = $null
try { $script:OriginalTitle = $Host.UI.RawUI.WindowTitle } catch { }

function Set-TerminalTitle {
    param([string]$Text)

    try { $Host.UI.RawUI.WindowTitle = $Text } catch { }
}

function Get-TerminalTitle {
    param([object[]]$Choices, [string]$Task, [string[]]$Enabled)

    $label = $Task
    foreach ($choice in $Choices) {
        if ($choice.Name -eq $Task) {
            $label = $choice.Title
            break
        }
    }

    $featureText = '无特性'
    if ($Enabled -and $Enabled.Count -gt 0) { $featureText = $Enabled -join ',' }

    return ('assistant · {0} · {1}' -f $label, $featureText)
}

function Select-TaskInteractive {
    param([object[]]$Choices)

    Write-Host ''
    Write-Host '未提供 -Task，进入交互选择（回车 = 1. dev）' -ForegroundColor Cyan
    Write-Host ''
    for ($i = 0; $i -lt $Choices.Count; $i++) {
        Write-Host ('  {0}) {1,-10} {2}' -f ($i + 1), $Choices[$i].Name, $Choices[$i].Desc)
    }
    Write-Host ''

    $answer = $null
    try {
        $answer = Read-Host ('请选择任务 [1-{0} / 名称，回车=1]' -f $Choices.Count)
    } catch {
        Write-Host '   无法读取输入，使用默认 dev' -ForegroundColor Yellow
        return $Choices[0].Name
    }

    if ([string]::IsNullOrWhiteSpace($answer)) { return $Choices[0].Name }
    $answer = $answer.Trim()

    if ($answer -match '^\d+$') {
        $index = [int]$answer - 1
        if ($index -ge 0 -and $index -lt $Choices.Count) { return $Choices[$index].Name }
    }
    foreach ($choice in $Choices) {
        if ($choice.Name -eq $answer.ToLowerInvariant()) { return $choice.Name }
    }

    Write-Host ("!! 无法识别 '{0}'，使用默认 {1}" -f $answer, $Choices[0].Name) -ForegroundColor Yellow
    return $Choices[0].Name
}

function Select-FeaturesInteractive {
    param($Table)

    $defaults = @($Table | Where-Object { $_.Default } | ForEach-Object { $_.Id })
    $defaultText = '(无)'
    if ($defaults.Count -gt 0) { $defaultText = $defaults -join ', ' }

    Write-Host ''
    Write-Host ("特性清单 : {0}" -f (($Table | ForEach-Object { $_.Id }) -join ', ')) -ForegroundColor Cyan
    Write-Host ("默认启用 : {0}" -f $defaultText) -ForegroundColor Cyan

    $answer = $null
    try {
        $answer = Read-Host "请选择特性 [回车=默认 / '-'=全关 / 逗号分隔=覆盖]"
    } catch {
        Write-Host '   无法读取输入，沿用默认特性' -ForegroundColor Yellow
        return $null
    }

    if ([string]::IsNullOrWhiteSpace($answer)) { return $null }
    $answer = $answer.Trim()

    if ($answer -eq '-') {
        return [pscustomobject]@{
            Features = $null
            Without  = (($Table | ForEach-Object { $_.Id }) -join ',')
        }
    }
    return [pscustomobject]@{ Features = $answer; Without = $null }
}

function Set-Utf8Console {
    $utf8 = New-Object System.Text.UTF8Encoding $false
    [Console]::OutputEncoding = $utf8
    [Console]::InputEncoding = $utf8
    $global:OutputEncoding = $utf8
    $env:PYTHONIOENCODING = 'utf-8'
    $env:PYTHONUTF8 = '1'
    $env:CARGO_TERM_COLOR = 'always'
    $env:RUST_BACKTRACE = '1'
    try { & chcp.com 65001 *> $null } catch { }
}

function Get-CargoFeatures {
    $lines = Get-Content -LiteralPath $ManifestPath -Encoding UTF8
    $blockLines = @()
    $inFeatures = $false

    foreach ($raw in $lines) {
        $line = ($raw -replace '#.*$', '').Trim()
        if ($line -match '^\[') {
            if ($inFeatures) { break }
            $inFeatures = $line -match '^\[features\]'
            continue
        }
        if ($inFeatures) { $blockLines += $line }
    }

    if ($blockLines.Count -eq 0) {
        throw "在 $ManifestPath 中找不到 [features] 段"
    }

    $block = $blockLines -join "`n"
    $declared = @{}
    foreach ($match in [regex]::Matches($block, '(?m)^(?<name>[A-Za-z0-9_-]+)\s*=\s*\[(?<deps>[^\]]*)\]')) {
        $name = $match.Groups['name'].Value
        $deps = @(
            $match.Groups['deps'].Value -split ',' |
                ForEach-Object { $_.Trim().Trim('"') } |
                Where-Object { $_ -ne '' }
        )
        $declared[$name] = $deps
    }

    $defaults = @()
    if ($declared.ContainsKey('default')) { $defaults = $declared['default'] }

    return [pscustomobject]@{
        Declared = $declared
        Defaults = $defaults
    }
}

function Get-FeatureTable {
    if (-not (Test-Path -LiteralPath $FeatureManifestPath)) {
        throw "找不到特性清单 $FeatureManifestPath"
    }

    $manifest = Get-Content -LiteralPath $FeatureManifestPath -Encoding UTF8 -Raw | ConvertFrom-Json
    $table = @()

    foreach ($property in $manifest.PSObject.Properties) {
        $id = $property.Name
        $spec = $property.Value

        if (-not $spec.cargoFeature) { throw "特性 '$id' 缺少 cargoFeature" }
        if (-not $spec.env) { throw "特性 '$id' 缺少 env" }
        if ($spec.env -notmatch '^[A-Z][A-Z0-9_]*$') {
            throw "特性 '$id' 的 env '$($spec.env)' 必须是大写字母/数字/下划线"
        }

        $table += [pscustomobject]@{
            Id           = $id
            Default      = [bool]$spec.default
            CargoFeature = [string]$spec.cargoFeature
            Env          = [string]$spec.env
        }
    }

    if ($table.Count -eq 0) { throw "$FeatureManifestPath 未声明任何特性" }

    return @($table | Sort-Object -Property Id)
}

function Assert-FeatureConsistency {
    param($Table)

    $cargo = Get-CargoFeatures
    $problems = @()

    foreach ($feature in $Table) {
        if (-not $cargo.Declared.ContainsKey($feature.CargoFeature)) {
            $problems += ("清单特性 '{0}' 的 cargoFeature '{1}' 在 Cargo.toml [features] 中未声明" -f $feature.Id, $feature.CargoFeature)
        }
    }

    foreach ($name in $cargo.Declared.Keys) {
        if ($name -eq 'default') { continue }
        if (-not ($Table | Where-Object { $_.CargoFeature -eq $name })) {
            $problems += ("Cargo.toml [features] 中的 '{0}' 未出现在 {1} 中" -f $name, $FeatureManifestPath)
        }
    }

    $manifestDefaults = @($Table | Where-Object { $_.Default } | ForEach-Object { $_.CargoFeature } | Sort-Object)
    $cargoDefaults = @($cargo.Defaults | Sort-Object)
    if (($manifestDefaults -join ',') -ne ($cargoDefaults -join ',')) {
        $problems += ("默认启用集合不一致：清单 = [{0}]，Cargo.toml default = [{1}]" -f ($manifestDefaults -join ','), ($cargoDefaults -join ','))
    }

    if ($problems.Count -gt 0) {
        Write-Host '!! 特性清单与 Cargo.toml 不一致' -ForegroundColor Red
        foreach ($problem in $problems) { Write-Host "   - $problem" -ForegroundColor Yellow }
        throw '特性清单校验失败（config/features.json ↔ src-tauri/Cargo.toml）'
    }
}

function Split-List {
    param([string]$Value)
    if ([string]::IsNullOrWhiteSpace($Value)) { return @() }
    return @($Value -split ',' | ForEach-Object { $_.Trim() } | Where-Object { $_ -ne '' })
}

function Resolve-Features {
    param($Table, [string]$Include, [string]$Exclude)

    $known = @($Table | ForEach-Object { $_.Id })

    if ([string]::IsNullOrWhiteSpace($Include)) {
        $enabled = @($Table | Where-Object { $_.Default } | ForEach-Object { $_.Id })
    } else {
        $enabled = Split-List -Value $Include
    }

    $dropped = Split-List -Value $Exclude
    $enabled = @($enabled | Where-Object { $dropped -notcontains $_ })

    foreach ($id in $enabled) {
        if ($known -notcontains $id) {
            throw "未知特性 '$id'，可用值：$($known -join ', ')"
        }
    }
    foreach ($id in $dropped) {
        if ($known -notcontains $id) {
            throw "未知特性 '$id'，可用值：$($known -join ', ')"
        }
    }

    return @($enabled | Select-Object -Unique)
}

function Set-FeatureEnvironment {
    param($Table, [string[]]$Enabled)

    foreach ($feature in $Table) {
        $on = $Enabled -contains $feature.Id
        $value = '0'
        if ($on) { $value = '1' }
        Set-Item -Path ('env:' + $feature.Env) -Value $value
    }
}

function Assert-CargoConfigPresent {
    if (Test-Path -LiteralPath $CargoConfigPath) { return $true }

    Write-Host '!! 缺少 src-tauri/.cargo/config.toml' -ForegroundColor Red
    Write-Host '   该文件提供 OPENSSL_DIR / OPENSSL_STATIC；缺失时 libsqlite3-sys（bundled-sqlcipher）构建会失败。' -ForegroundColor Yellow
    Write-Host ("   执行：Copy-Item '{0}' '{1}'，再把 OPENSSL_DIR 填成本机 vcpkg 路径。" -f $CargoConfigExamplePath, $CargoConfigPath) -ForegroundColor Yellow
    return $false
}

function Get-CargoConfigChain {
    $chain = @()
    $dir = $SrcTauri
    while ($dir) {
        $candidate = Join-Path $dir '.cargo/config.toml'
        if (Test-Path -LiteralPath $candidate) { $chain += $candidate }
        $parent = Split-Path -Parent $dir
        if (-not $parent -or $parent -eq $dir) { break }
        $dir = $parent
    }
    return $chain
}

function Get-EnabledCargoFeatures {
    param($Table, [string[]]$Enabled)

    return @(
        $Table |
            Where-Object { $Enabled -contains $_.Id } |
            ForEach-Object { $_.CargoFeature }
    )
}

function Get-CargoFeatureArgs {
    param([string[]]$CargoFeatures)

    $args = @('--no-default-features')
    if ($CargoFeatures.Count -gt 0) {
        $args += @('--features', ($CargoFeatures -join ','))
    }
    if ($Target) { $args += @('--target', $Target) }
    return $args
}

function Get-TauriArgs {
    param([string[]]$CargoFeatures, [string]$SubCommand, [string[]]$TauriOptions, [string[]]$RunnerOptions)

    $args = @($SubCommand)
    if ($Target) { $args += @('--target', $Target) }
    if ($CargoFeatures.Count -gt 0) {
        $args += @('--features', ($CargoFeatures -join ','))
    }
    if ($TauriOptions) { $args += $TauriOptions }

    $args += @('--')
    $args += @('--no-default-features')
    if ($RunnerOptions) { $args += $RunnerOptions }
    return $args
}

function Invoke-Step {
    param([string]$Label, [string]$Command, [string[]]$Arguments, [string]$WorkingDirectory)

    Write-Host ''
    Write-Host ">> $Label" -ForegroundColor Cyan
    Write-Host "   cwd: $WorkingDirectory" -ForegroundColor DarkGray
    Write-Host "   $Command $($Arguments -join ' ')" -ForegroundColor DarkGray
    Write-Host ''

    Push-Location -LiteralPath $WorkingDirectory
    try {
        & $Command @Arguments
        $code = $LASTEXITCODE
    } finally {
        Pop-Location
    }

    if ($code -ne 0) {
        Write-Host "!! 失败：$Label（退出码 $code）" -ForegroundColor Red
    }
    return $code
}

Set-Utf8Console

$interactive = ($PSBoundParameters.Count -eq 0)
if ($interactive) {
    Set-TerminalTitle 'assistant · 选择任务…'
    $Task = Select-TaskInteractive -Choices $TaskChoices
}

$needsRust = @($TaskChoices | Where-Object { $_.Name -eq $Task -and $_.NeedsRust }).Count -gt 0
if ($needsRust -and -not (Assert-CargoConfigPresent)) {
    Write-Host '   当前任务需要编译 Rust，已中止。' -ForegroundColor Red
    Set-TerminalTitle 'assistant · 已中止'
    exit 1
}

$table = Get-FeatureTable
Assert-FeatureConsistency -Table $table

if ($interactive) {
    Set-TerminalTitle (Get-TerminalTitle -Choices $TaskChoices -Task $Task -Enabled @('选择特性…'))
    $featureChoice = Select-FeaturesInteractive -Table $table
    if ($featureChoice) {
        if ($featureChoice.Features) { $Features = $featureChoice.Features }
        if ($featureChoice.Without) { $Without = $featureChoice.Without }
    }
}

$enabled = Resolve-Features -Table $table -Include $Features -Exclude $Without
Set-FeatureEnvironment -Table $table -Enabled $enabled

Set-TerminalTitle (Get-TerminalTitle -Choices $TaskChoices -Task $Task -Enabled $enabled)

$cargoFeatures = Get-EnabledCargoFeatures -Table $table -Enabled $enabled
$cargoFeatureArgs = Get-CargoFeatureArgs -CargoFeatures $cargoFeatures
$cargoConfigChain = Get-CargoConfigChain

$tauriOptions = @()
if ($Task -eq 'build' -and $NoBundle) { $tauriOptions += '--no-bundle' }
if ($ExtraArgs) { $tauriOptions += $ExtraArgs }
$tauriArgs = Get-TauriArgs -CargoFeatures $cargoFeatures -SubCommand $Task -TauriOptions $tauriOptions -RunnerOptions $CargoArgs

Write-Host ''
Write-Host '======== 构建配置 ========' -ForegroundColor Green
Write-Host ("  任务      : {0}" -f $Task)
Write-Host ("  启用特性  : {0}" -f $(if ($enabled.Count -gt 0) { $enabled -join ', ' } else { '(无)' }))
foreach ($feature in $table) {
    $state = '0'
    if ($enabled -contains $feature.Id) { $state = '1' }
    Write-Host ("  环境变量  : {0}={1}" -f $feature.Env, $state)
}
Write-Host ("  cargo 参数: {0}" -f ($cargoFeatureArgs -join ' '))
if ($Target) { Write-Host ("  target    : {0}" -f $Target) }
Write-Host ("  cargo cwd : {0}" -f $SrcTauri)
Write-Host ("  cargo 配置: {0}" -f $(if ($cargoConfigChain.Count -gt 0) { $cargoConfigChain -join ' + ' } else { '(无)' }))
if ($Task -eq 'dev' -or $Task -eq 'build') {
    Write-Host ("  tauri 命令: npm run tauri -- {0}" -f ($tauriArgs -join ' '))
}
Write-Host '========================='
Write-Host ''

switch ($Task) {
    'dev' {
        $code = Invoke-Step -Label 'tauri dev' -Command 'npm.cmd' -Arguments (@('run', 'tauri', '--') + $tauriArgs) -WorkingDirectory $RepoRoot
    }
    'build' {
        $code = Invoke-Step -Label 'tauri build' -Command 'npm.cmd' -Arguments (@('run', 'tauri', '--') + $tauriArgs) -WorkingDirectory $RepoRoot
    }
    'check' {
        $checkArgs = @('check') + $cargoFeatureArgs
        if ($Release) { $checkArgs += '--release' }
        if ($ExtraArgs) { $checkArgs += $ExtraArgs }
        if ($CargoArgs) { $checkArgs += $CargoArgs }
        $code = Invoke-Step -Label 'cargo check' -Command 'cargo.exe' -Arguments $checkArgs -WorkingDirectory $SrcTauri
    }
    'test' {
        $testArgs = @('test') + $cargoFeatureArgs
        if ($Release) { $testArgs += '--release' }
        if ($ExtraArgs) { $testArgs += $ExtraArgs }
        if ($CargoArgs) { $testArgs += $CargoArgs }
        $code = Invoke-Step -Label 'cargo test' -Command 'cargo.exe' -Arguments $testArgs -WorkingDirectory $SrcTauri
    }
    'lint' {
        $code = Invoke-Step -Label 'npm run lint' -Command 'npm.cmd' -Arguments (@('run', 'lint', '--') + $ExtraArgs) -WorkingDirectory $RepoRoot
    }
    'typecheck' {
        $code = Invoke-Step -Label 'npm run typecheck' -Command 'npm.cmd' -Arguments (@('run', 'typecheck', '--') + $ExtraArgs) -WorkingDirectory $RepoRoot
    }
    'clean' {
        $code = Invoke-Step -Label 'cargo clean' -Command 'cargo.exe' -Arguments (@('clean') + $ExtraArgs) -WorkingDirectory $SrcTauri
    }
}

if ($script:OriginalTitle) { Set-TerminalTitle $script:OriginalTitle }

exit $code