param(
  [Parameter(Mandatory = $true)]
  [string]$SourcePath,

  [string]$OutputPath = ""
)

$ErrorActionPreference = "Stop"
Add-Type -AssemblyName System.Drawing

if ([string]::IsNullOrWhiteSpace($OutputPath)) {
  $OutputPath = Join-Path $PSScriptRoot "..\..\src\assets\keynest-mark.png"
}

function Get-ClampedByte([double]$Value) {
  return [byte][Math]::Round([Math]::Max(0, [Math]::Min(255, $Value)))
}

$resolvedSource = (Resolve-Path -LiteralPath $SourcePath).Path
$resolvedOutput = [IO.Path]::GetFullPath($OutputPath)
$outputDirectory = Split-Path -Parent $resolvedOutput
New-Item -ItemType Directory -Force -Path $outputDirectory | Out-Null

$source = $null
$crop = $null
$cleaned = $null
$output = $null
$graphics = $null

try {
  $source = [Drawing.Bitmap]::FromFile($resolvedSource)

  if ($source.Width -ne 1448 -or $source.Height -ne 1086) {
    throw "Expected the approved 1448x1086 source artwork. Received $($source.Width)x$($source.Height)."
  }

  # Contains only the large symbol on the light presentation, with matte around it.
  $cropRectangle = [Drawing.Rectangle]::new(120, 210, 480, 350)
  $crop = $source.Clone(
    $cropRectangle,
    [Drawing.Imaging.PixelFormat]::Format32bppArgb
  )
  $cleaned = [Drawing.Bitmap]::new(
    $crop.Width,
    $crop.Height,
    [Drawing.Imaging.PixelFormat]::Format32bppArgb
  )

  $minX = $cleaned.Width
  $minY = $cleaned.Height
  $maxX = -1
  $maxY = -1
  $transparentDistance = 18.0
  $opaqueDistance = 245.0

  for ($y = 0; $y -lt $crop.Height; $y++) {
    for ($x = 0; $x -lt $crop.Width; $x++) {
      $pixel = $crop.GetPixel($x, $y)
      $redDistance = 255 - $pixel.R
      $greenDistance = 255 - $pixel.G
      $blueDistance = 255 - $pixel.B
      $distance = [Math]::Sqrt(
        ($redDistance * $redDistance) +
        ($greenDistance * $greenDistance) +
        ($blueDistance * $blueDistance)
      )

      if ($distance -le $transparentDistance) {
        $alpha = 0
      } elseif ($distance -ge $opaqueDistance) {
        $alpha = 255
      } else {
        $alpha = Get-ClampedByte (
          (($distance - $transparentDistance) /
            ($opaqueDistance - $transparentDistance)) * 255
        )
      }

      if ($alpha -eq 0) {
        $cleaned.SetPixel($x, $y, [Drawing.Color]::Transparent)
        continue
      }

      # Remove the white matte from antialiased edge pixels.
      $alphaRatio = $alpha / 255.0
      $red = Get-ClampedByte (
        ($pixel.R - (255 * (1 - $alphaRatio))) / $alphaRatio
      )
      $green = Get-ClampedByte (
        ($pixel.G - (255 * (1 - $alphaRatio))) / $alphaRatio
      )
      $blue = Get-ClampedByte (
        ($pixel.B - (255 * (1 - $alphaRatio))) / $alphaRatio
      )
      $cleaned.SetPixel(
        $x,
        $y,
        [Drawing.Color]::FromArgb($alpha, $red, $green, $blue)
      )

      if ($alpha -ge 16) {
        $minX = [Math]::Min($minX, $x)
        $minY = [Math]::Min($minY, $y)
        $maxX = [Math]::Max($maxX, $x)
        $maxY = [Math]::Max($maxY, $y)
      }
    }
  }

  if ($maxX -lt $minX -or $maxY -lt $minY) {
    throw "No visible logo pixels were extracted."
  }

  $trimRectangle = [Drawing.Rectangle]::new(
    $minX,
    $minY,
    ($maxX - $minX + 1),
    ($maxY - $minY + 1)
  )
  $maximumWidth = 820.0
  $maximumHeight = 680.0
  $scale = [Math]::Min(
    $maximumWidth / $trimRectangle.Width,
    $maximumHeight / $trimRectangle.Height
  )
  $destinationWidth = [int][Math]::Round($trimRectangle.Width * $scale)
  $destinationHeight = [int][Math]::Round($trimRectangle.Height * $scale)
  $destinationRectangle = [Drawing.Rectangle]::new(
    [int][Math]::Round((1024 - $destinationWidth) / 2),
    [int][Math]::Round((1024 - $destinationHeight) / 2),
    $destinationWidth,
    $destinationHeight
  )

  $output = [Drawing.Bitmap]::new(
    1024,
    1024,
    [Drawing.Imaging.PixelFormat]::Format32bppArgb
  )
  $graphics = [Drawing.Graphics]::FromImage($output)
  $graphics.Clear([Drawing.Color]::Transparent)
  $graphics.CompositingMode = [Drawing.Drawing2D.CompositingMode]::SourceCopy
  $graphics.CompositingQuality = [Drawing.Drawing2D.CompositingQuality]::HighQuality
  $graphics.InterpolationMode = [Drawing.Drawing2D.InterpolationMode]::HighQualityBicubic
  $graphics.PixelOffsetMode = [Drawing.Drawing2D.PixelOffsetMode]::HighQuality
  $graphics.SmoothingMode = [Drawing.Drawing2D.SmoothingMode]::HighQuality
  $graphics.DrawImage(
    $cleaned,
    $destinationRectangle,
    $trimRectangle,
    [Drawing.GraphicsUnit]::Pixel
  )
  $output.Save($resolvedOutput, [Drawing.Imaging.ImageFormat]::Png)
} finally {
  if ($graphics) { $graphics.Dispose() }
  if ($output) { $output.Dispose() }
  if ($cleaned) { $cleaned.Dispose() }
  if ($crop) { $crop.Dispose() }
  if ($source) { $source.Dispose() }
}

Write-Output "Created $resolvedOutput"
