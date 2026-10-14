$ErrorActionPreference = "Stop"

Add-Type -AssemblyName System.Drawing

$root = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
$iconsDir = Join-Path $root "src-tauri\icons"
$installerDir = Join-Path $root "src-tauri\installer"
$uiAssetsDir = Join-Path $root "src\assets"

New-Item -ItemType Directory -Force -Path $iconsDir, $installerDir, $uiAssetsDir | Out-Null

$sourcePoints = @(
  @(365,48), @(307,53), @(252,78), @(210,119), @(174,165),
  @(151,220), @(145,278), @(156,330), @(181,376), @(218,411),
  @(262,437), @(310,449), @(358,446), @(400,427), @(430,402),
  @(447,378), @(421,389), @(393,395), @(363,397), @(333,391),
  @(305,381), @(278,365), @(253,344), @(232,319), @(215,290),
  @(202,258), @(198,224), @(200,190), @(208,157), @(222,127),
  @(242,99), @(265,76), @(291,59), @(322,48)
)

function New-Canvas([int]$width, [int]$height, [bool]$alpha = $true) {
  $format = if ($alpha) {
    [System.Drawing.Imaging.PixelFormat]::Format32bppArgb
  } else {
    [System.Drawing.Imaging.PixelFormat]::Format24bppRgb
  }
  [System.Drawing.Bitmap]::new($width, $height, $format)
}

function Configure-Graphics([System.Drawing.Graphics]$graphics) {
  $graphics.CompositingQuality = [System.Drawing.Drawing2D.CompositingQuality]::HighQuality
  $graphics.InterpolationMode = [System.Drawing.Drawing2D.InterpolationMode]::HighQualityBicubic
  $graphics.SmoothingMode = [System.Drawing.Drawing2D.SmoothingMode]::AntiAlias
  $graphics.PixelOffsetMode = [System.Drawing.Drawing2D.PixelOffsetMode]::HighQuality
}

function New-CrescentPath([System.Drawing.RectangleF]$bounds) {
  [float]$minX = 68
  [float]$maxX = 455
  [float]$minY = 35
  [float]$maxY = 455
  [float]$sourceW = $maxX - $minX
  [float]$sourceH = $maxY - $minY
  [float]$scale = [Math]::Min($bounds.Width / $sourceW, $bounds.Height / $sourceH)
  [float]$drawW = $sourceW * $scale
  [float]$drawH = $sourceH * $scale
  [float]$offsetX = $bounds.X + (($bounds.Width - $drawW) / 2)
  [float]$offsetY = $bounds.Y + (($bounds.Height - $drawH) / 2)

  $points = New-Object 'System.Collections.Generic.List[System.Drawing.PointF]'
  foreach ($pair in $sourcePoints) {
    [float]$x = $offsetX + (($pair[0] - $minX) * $scale)
    [float]$y = $offsetY + (($pair[1] - $minY) * $scale)
    $points.Add([System.Drawing.PointF]::new($x, $y))
  }

  $path = [System.Drawing.Drawing2D.GraphicsPath]::new()
  $path.AddClosedCurve($points.ToArray(), 0.18)
  return $path
}

function New-SparklePath([System.Drawing.RectangleF]$bounds) {
  [float]$minX = 68
  [float]$maxX = 455
  [float]$minY = 35
  [float]$maxY = 455
  [float]$sourceW = $maxX - $minX
  [float]$sourceH = $maxY - $minY
  [float]$scale = [Math]::Min($bounds.Width / $sourceW, $bounds.Height / $sourceH)
  [float]$drawW = $sourceW * $scale
  [float]$drawH = $sourceH * $scale
  [float]$offsetX = $bounds.X + (($bounds.Width - $drawW) / 2)
  [float]$offsetY = $bounds.Y + (($bounds.Height - $drawH) / 2)
  $star = @(@(412,106), @(418,126), @(448,141), @(418,157), @(412,178), @(406,157), @(376,141), @(406,126))
  $points = New-Object 'System.Collections.Generic.List[System.Drawing.PointF]'
  foreach ($pair in $star) {
    [float]$x = $offsetX + (($pair[0] - $minX) * $scale)
    [float]$y = $offsetY + (($pair[1] - $minY) * $scale)
    $points.Add([System.Drawing.PointF]::new($x, $y))
  }
  $path = [System.Drawing.Drawing2D.GraphicsPath]::new()
  $path.AddPolygon($points.ToArray())
  return $path
}

function Draw-Crescent(
  [System.Drawing.Graphics]$graphics,
  [System.Drawing.RectangleF]$bounds,
  [System.Drawing.Color]$start,
  [System.Drawing.Color]$end
) {
  Configure-Graphics $graphics
  $path = New-CrescentPath $bounds
  $sparkle = New-SparklePath $bounds
  $brush = [System.Drawing.Drawing2D.LinearGradientBrush]::new($bounds, $start, $end, 45.0)
  [float]$minX = 68
  [float]$maxX = 455
  [float]$minY = 35
  [float]$maxY = 455
  [float]$scale = [Math]::Min($bounds.Width / ($maxX - $minX), $bounds.Height / ($maxY - $minY))
  [float]$drawW = ($maxX - $minX) * $scale
  [float]$drawH = ($maxY - $minY) * $scale
  [float]$offsetX = $bounds.X + (($bounds.Width - $drawW) / 2)
  [float]$offsetY = $bounds.Y + (($bounds.Height - $drawH) / 2)
  [float]$dotX = $offsetX + ((373 - $minX) * $scale)
  [float]$dotY = $offsetY + ((190 - $minY) * $scale)
  [float]$dotRadius = 6 * $scale
  try {
    $graphics.FillPath($brush, $path)
    $graphics.FillPath($brush, $sparkle)
    $graphics.FillEllipse($brush, $dotX - $dotRadius, $dotY - $dotRadius, 2 * $dotRadius, 2 * $dotRadius)
  } finally {
    $brush.Dispose()
    $sparkle.Dispose()
    $path.Dispose()
  }
}

function New-CrescentBitmap(
  [int]$size,
  [System.Drawing.Color]$start,
  [System.Drawing.Color]$end
) {
  $bitmap = New-Canvas $size $size $true
  $graphics = [System.Drawing.Graphics]::FromImage($bitmap)
  try {
    $graphics.Clear([System.Drawing.Color]::Transparent)
    [float]$pad = [Math]::Max(2, $size * 0.10)
    $bounds = [System.Drawing.RectangleF]::new($pad, $pad, $size - (2 * $pad), $size - (2 * $pad))
    Draw-Crescent $graphics $bounds $start $end
  } finally {
    $graphics.Dispose()
  }
  return $bitmap
}

function Save-CrescentPng(
  [string]$path,
  [int]$size,
  [System.Drawing.Color]$start,
  [System.Drawing.Color]$end
) {
  $bitmap = New-CrescentBitmap $size $start $end
  try {
    $bitmap.Save($path, [System.Drawing.Imaging.ImageFormat]::Png)
  } finally {
    $bitmap.Dispose()
  }
}

function Get-CrescentPngBytes([int]$size) {
  $bitmap = New-CrescentBitmap $size $violetStart $violetEnd
  $stream = [System.IO.MemoryStream]::new()
  try {
    $bitmap.Save($stream, [System.Drawing.Imaging.ImageFormat]::Png)
    return $stream.ToArray()
  } finally {
    $stream.Dispose()
    $bitmap.Dispose()
  }
}

function Write-Ico([string]$path, [int[]]$sizes) {
  $images = @()
  foreach ($size in $sizes) {
    $images += [pscustomobject]@{ Size = $size; Bytes = Get-CrescentPngBytes $size }
  }

  $file = [System.IO.File]::Create($path)
  $writer = [System.IO.BinaryWriter]::new($file)
  try {
    $writer.Write([UInt16]0)
    $writer.Write([UInt16]1)
    $writer.Write([UInt16]$images.Count)
    [UInt32]$offset = 6 + (16 * $images.Count)
    foreach ($image in $images) {
      $dimension = if ($image.Size -ge 256) { [byte]0 } else { [byte]$image.Size }
      $writer.Write($dimension)
      $writer.Write($dimension)
      $writer.Write([byte]0)
      $writer.Write([byte]0)
      $writer.Write([UInt16]1)
      $writer.Write([UInt16]32)
      $writer.Write([UInt32]$image.Bytes.Length)
      $writer.Write([UInt32]$offset)
      $offset += [UInt32]$image.Bytes.Length
    }
    foreach ($image in $images) { $writer.Write([byte[]]$image.Bytes) }
  } finally {
    $writer.Dispose()
    $file.Dispose()
  }
}

function New-DarkGradientBrush([System.Drawing.Rectangle]$rect, [float]$angle = 90.0) {
  [System.Drawing.Drawing2D.LinearGradientBrush]::new(
    $rect,
    [System.Drawing.Color]::FromArgb(5, 5, 10),
    [System.Drawing.Color]::FromArgb(32, 9, 67),
    $angle
  )
}

function Save-Header([string]$path) {
  $bitmap = New-Canvas 150 57 $false
  $graphics = [System.Drawing.Graphics]::FromImage($bitmap)
  $rect = [System.Drawing.Rectangle]::new(0, 0, 150, 57)
  $brush = New-DarkGradientBrush $rect 0
  $violet = [System.Drawing.Pen]::new([System.Drawing.Color]::FromArgb(139, 92, 246), 2)
  $cyan = [System.Drawing.Pen]::new([System.Drawing.Color]::FromArgb(34, 211, 238), 1)
  $red = [System.Drawing.Pen]::new([System.Drawing.Color]::FromArgb(244, 63, 94), 1)
  try {
    Configure-Graphics $graphics
    $graphics.FillRectangle($brush, $rect)
    $graphics.DrawLine($cyan, 0, 2, 60, 2)
    $graphics.DrawLine($violet, 0, 55, 150, 55)
    $graphics.DrawLine($red, 110, 3, 150, 3)
    $bounds = [System.Drawing.RectangleF]::new(100, 3, 50, 50)
    Draw-Crescent $graphics $bounds $violetStart $violetEnd
    $bitmap.Save($path, [System.Drawing.Imaging.ImageFormat]::Bmp)
  } finally {
    $red.Dispose(); $cyan.Dispose(); $violet.Dispose(); $brush.Dispose()
    $graphics.Dispose(); $bitmap.Dispose()
  }
}

function Save-Sidebar([string]$path) {
  $bitmap = New-Canvas 164 314 $false
  $graphics = [System.Drawing.Graphics]::FromImage($bitmap)
  $rect = [System.Drawing.Rectangle]::new(0, 0, 164, 314)
  $brush = New-DarkGradientBrush $rect 90
  $violet = [System.Drawing.Pen]::new([System.Drawing.Color]::FromArgb(139, 92, 246), 3)
  $cyan = [System.Drawing.Pen]::new([System.Drawing.Color]::FromArgb(34, 211, 238), 1)
  $red = [System.Drawing.Pen]::new([System.Drawing.Color]::FromArgb(244, 63, 94), 1)
  try {
    Configure-Graphics $graphics
    $graphics.FillRectangle($brush, $rect)
    $graphics.DrawLine($violet, 2, 0, 2, 314)
    $graphics.DrawLine($cyan, 7, 0, 7, 98)
    $graphics.DrawLine($red, 7, 216, 7, 314)
    $bounds = [System.Drawing.RectangleF]::new(16, 64, 132, 132)
    Draw-Crescent $graphics $bounds $violetStart $violetEnd
    $bitmap.Save($path, [System.Drawing.Imaging.ImageFormat]::Bmp)
  } finally {
    $red.Dispose(); $cyan.Dispose(); $violet.Dispose(); $brush.Dispose()
    $graphics.Dispose(); $bitmap.Dispose()
  }
}

$violetStart = [System.Drawing.Color]::FromArgb(180, 92, 255)
$violetEnd = [System.Drawing.Color]::FromArgb(86, 35, 214)
$cyanStart = [System.Drawing.Color]::FromArgb(82, 246, 255)
$cyanEnd = [System.Drawing.Color]::FromArgb(0, 167, 215)
$redStart = [System.Drawing.Color]::FromArgb(255, 95, 112)
$redEnd = [System.Drawing.Color]::FromArgb(211, 19, 66)

Save-CrescentPng (Join-Path $iconsDir "dusk-logo.png") 512 $violetStart $violetEnd
Save-CrescentPng (Join-Path $iconsDir "dusk-logo-violet.png") 512 $violetStart $violetEnd
Save-CrescentPng (Join-Path $iconsDir "dusk-logo-cyan.png") 512 $cyanStart $cyanEnd
Save-CrescentPng (Join-Path $iconsDir "dusk-logo-red.png") 512 $redStart $redEnd

Save-CrescentPng (Join-Path $uiAssetsDir "dusk-logo.png") 512 $violetStart $violetEnd
Save-CrescentPng (Join-Path $uiAssetsDir "dusk-logo-cyan.png") 512 $cyanStart $cyanEnd
Save-CrescentPng (Join-Path $uiAssetsDir "dusk-logo-red.png") 512 $redStart $redEnd

Save-CrescentPng (Join-Path $iconsDir "32x32.png") 32 $violetStart $violetEnd
Save-CrescentPng (Join-Path $iconsDir "128x128.png") 128 $violetStart $violetEnd
Save-CrescentPng (Join-Path $iconsDir "128x128@2x.png") 256 $violetStart $violetEnd
Save-CrescentPng (Join-Path $iconsDir "icon.png") 256 $violetStart $violetEnd
Write-Ico (Join-Path $iconsDir "icon.ico") @(16, 24, 32, 48, 64, 128, 256)

Save-Header (Join-Path $installerDir "header.bmp")
Save-Sidebar (Join-Path $installerDir "sidebar.bmp")

Write-Host "Dusk crescent branding generated with transparent primary/cyan/red assets."
