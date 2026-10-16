<#
.SYNOPSIS
  I16 naming migration verification script.
  Scans for residual snake_case fields, Row suffixes, backend DTO contract violations,
  and deleted original protocol files. Re-runnable; exit code 0 = all pass.
.EXAMPLE
  ./scripts/verify-naming-migration.ps1
#>

$ErrorActionPreference = "Stop"
$root = Resolve-Path (Join-Path $PSScriptRoot "..")
$srcDir = Join-Path $root "src"
$protocolDir = Join-Path $root "src-tauri" "src" "protocol"

$global:failCount = 0

function Assert-CountZero {
    param(
        [string]$Label,
        [int]$Count,
        [string[]]$Lines
    )
    if ($Count -eq 0) {
        Write-Host "  PASS: $Label" -ForegroundColor Green
    } else {
        Write-Host "  FAIL: $Label ($Count hits)" -ForegroundColor Red
        $Lines | ForEach-Object { Write-Host "    $_" -ForegroundColor Yellow }
        $script:failCount += $Count
    }
}

# ── K4: Frontend snake_case residual scan ──────────────────────────
Write-Host "`n[K4] Frontend snake_case residual scan" -ForegroundColor Cyan

$snakePatterns = @(
    'conversation_id', 'attempt_id', 'message_id', 'parent_id',
    'provider_id', 'model_id', 'title_source', 'system_prompt',
    'sort_order', 'created_at', 'updated_at', 'is_pinned', 'is_full', 'is_builtin',
    'delta_text', 'delta_reasoning', 'delta_tool_call_id', 'delta_tool_call_name',
    'delta_tool_call_arguments', 'delta_retry', 'finish_reason', 'deleted_message_id',
    'prompt_tokens', 'completion_tokens', 'total_tokens',
    'cache_read_tokens', 'cache_write_tokens', 'context_window_tokens',
    'max_attempts', 'delay_ms', 'retry_after_ms', 'total_count', 'tool_calls_json',
    'base_url', 'api_key_masked', 'api_key_changed', 'default_model',
    'owned_by', 'context_window',
    'max_tokens', 'reasoning_enabled', 'reasoning_effort', 'presentation_rank',
    'primary_color', 'secondary_color', 'installed_path', 'stay_on_top', 'command_palette',
    'click_through', 'opacity_percent', 'scale_percent',
    'hide_on_pointer_hover', 'hover_delay_ms', 'idle_opacity_percent',
    'client_x', 'client_y',
    'rive_src', 'gif_src', 'svg_animated', 'gif_border_radius', 'rive_idle_pose_rotation'
)

$excludeFilePatterns = @(
    '*\__tests__\protocolContract.test.ts'
)

foreach ($pat in $snakePatterns) {
    $regex = "\b$pat\b"
    $allMatches = Get-ChildItem -Path $srcDir -Recurse -Include *.ts,*.tsx |
        Select-String -Pattern $regex -CaseSensitive

    $filtered = $allMatches | Where-Object {
        $line = $_.Line.Trim()
        if ($line -match '\.not\.toHaveProperty\(') { return $false }
        if ($line -match 'expect\(.*\)\.not\.toHaveProperty\(') { return $false }
        if ($line -match '"[^"]*_commands?"' -and $pat -match '_command') { return $false }
        if ($line -match '"list_conversations"' -or $line -match '"search_conversations"' -or $line -match '"get_conversation') { return $false }
        if ($line -match '"[a-z_]+_[a-z_]+"' -and $line -notmatch '\bvar\b' -and $line -notmatch '\bconst\b' -and $line -notmatch '\blet\b') {
            if ($line -match 't\(' -or $line -match 'i18n\.t\(') { return $false }
        }
        if ($line -match 't\("') { return $false }
        if ($line -match 'label:\s*"' -or $line -match 'saveMessage:\s*"' -or $line -match 'extra:\s*"' -or $line -match 'placeholder:\s*"') { return $false }
        if ($line -match '"settings\.' -or $line -match '"chat\.' -or $line -match '"orb\.') { return $false }
        if ($line -match '"[a-z]+\.[a-z_]+\.[a-z_]+"\s*[\),]') { return $false }
        return $true
    }

    $hitCount = ($filtered | Measure-Object).Count
    $hitLines = if ($hitCount -gt 0) { $filtered | ForEach-Object { "$($_.RelativePath):$($_.LineNumber): $($_.Line.Trim())" } } else { @() }
    Assert-CountZero -Label "snake_case '$pat'" -Count $hitCount -Lines $hitLines
}

# ── K5: Frontend Row suffix residual scan ──────────────────────────
Write-Host "`n[K5] Frontend Row suffix residual scan" -ForegroundColor Cyan

$rowPattern = 'MessageRow|ConversationRow|ConversationDetailRow|TokenUsageRow|RetryNotificationRow|TokenUsageSummaryRow|ProviderErrorPayloadRow|MousePositionRow|ScreenSizeRow'
$rowMatches = Get-ChildItem -Path $srcDir -Recurse -Include *.ts,*.tsx |
    Select-String -Pattern $rowPattern

$rowCount = ($rowMatches | Measure-Object).Count
$rowLines = if ($rowCount -gt 0) { $rowMatches | ForEach-Object { "$($_.RelativePath):$($_.LineNumber): $($_.Line.Trim())" } } else { @() }
Assert-CountZero -Label "Row suffix" -Count $rowCount -Lines $rowLines

# ── K6: Backend DTO contract scan ──────────────────────────────────
Write-Host "`n[K6] Backend DTO contract scan (no rename_all = snake_case)" -ForegroundColor Cyan

if (Test-Path $protocolDir) {
    $snakeCaseSerde = Get-ChildItem -Path $protocolDir -Recurse -Include *.rs |
        Select-String -Pattern 'rename_all\s*=\s*"snake_case"'

    $serdeCount = ($snakeCaseSerde | Measure-Object).Count
    $serdeLines = if ($serdeCount -gt 0) { $snakeCaseSerde | ForEach-Object { "$($_.RelativePath):$($_.LineNumber): $($_.Line.Trim())" } } else { @() }
    Assert-CountZero -Label "rename_all = snake_case" -Count $serdeCount -Lines $serdeLines
} else {
    Write-Host "  SKIP: protocol dir not found at $protocolDir" -ForegroundColor Yellow
}

# ── K7: Backend original protocol file scan ────────────────────────
Write-Host "`n[K7] Backend original protocol file scan" -ForegroundColor Cyan

$deletedFiles = @(
    'chat_events.rs',
    'chat_commands.rs',
    'setting_commands.rs',
    'setting_events.rs',
    'orb_commands.rs'
)

$expectedFiles = @('mod.rs', 'dto.rs')

foreach ($f in $deletedFiles) {
    $path = Join-Path $protocolDir $f
    if (Test-Path $path) {
        Write-Host "  FAIL: deleted file still exists: $path" -ForegroundColor Red
        $script:failCount++
    } else {
        Write-Host "  PASS: $f absent" -ForegroundColor Green
    }
}

$actualRsFiles = (Get-ChildItem -Path $protocolDir -Filter *.rs | Select-Object -ExpandProperty Name) | Sort-Object
$expectedSorted = $expectedFiles | Sort-Object
$diff = Compare-Object $actualRsFiles $expectedSorted -SyncWindow 0
if ($diff) {
    $extra = $diff | Where-Object { $_.SideIndicator -eq '<=' } | Select-Object -ExpandProperty InputObject
    if ($extra) {
        Write-Host "  FAIL: unexpected .rs files in protocol/: $($extra -join ', ')" -ForegroundColor Red
        $script:failCount += ($extra | Measure-Object).Count
    } else {
        Write-Host "  PASS: protocol/ contains exactly $($expectedFiles -join ', ')" -ForegroundColor Green
    }
} else {
    Write-Host "  PASS: protocol/ contains exactly $($expectedFiles -join ', ')" -ForegroundColor Green
}

# ── Summary ────────────────────────────────────────────────────────
Write-Host "`n═══════════════════════════════════════════════" -ForegroundColor White
if ($global:failCount -eq 0) {
    Write-Host "  ALL CHECKS PASSED" -ForegroundColor Green
    exit 0
} else {
    Write-Host "  $global:failCount FAILURE(S) DETECTED" -ForegroundColor Red
    exit 1
}