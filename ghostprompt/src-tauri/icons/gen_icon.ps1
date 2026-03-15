Add-Type -AssemblyName System.Drawing
$iconDir = "d:\program\code\workSpace-rust-new\GhostPrompt\ghostprompt\src-tauri\icons"

# 创建 128x128 PNG
$bmp = New-Object System.Drawing.Bitmap(128, 128)
$g = [System.Drawing.Graphics]::FromImage($bmp)
$g.FillRectangle([System.Drawing.Brushes]::CornflowerBlue, 0, 0, 128, 128)
$g.Dispose()
$bmp.Save("$iconDir\icon.png", [System.Drawing.Imaging.ImageFormat]::Png)

# 创建 ICO
$iconHandle = $bmp.GetHicon()
$icon = [System.Drawing.Icon]::FromHandle($iconHandle)
$ms = New-Object System.IO.MemoryStream
$icon.Save($ms)
[System.IO.File]::WriteAllBytes("$iconDir\icon.ico", $ms.ToArray())
$bmp.Dispose()

Write-Host "Icons created successfully"
dir $iconDir
