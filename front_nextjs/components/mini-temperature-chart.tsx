"use client"

interface MiniTemperatureChartProps {
  data: number[]
}

export function MiniTemperatureChart({ data }: MiniTemperatureChartProps) {
  // Normalizar datos para el gráfico
  const min = Math.min(...data)
  const max = Math.max(...data)
  const range = max - min || 1 // Evitar división por cero

  const normalizedData = data.map((value) => ({
    value,
    normalized: ((value - min) / range) * 100,
  }))

  // Crear path SVG
  const width = 120
  const height = 40
  const padding = 4
  const stepX = (width - padding * 2) / (data.length - 1)

  const pathData = normalizedData
    .map((point, index) => {
      const x = padding + index * stepX
      const y = height - padding - (point.normalized / 100) * (height - padding * 2)
      return `${index === 0 ? "M" : "L"} ${x} ${y}`
    })
    .join(" ")

  return (
    <svg width={width} height={height} viewBox={`0 0 ${width} ${height}`} className="overflow-visible">
      <path
        d={pathData}
        fill="none"
        stroke="currentColor"
        strokeWidth="2"
        strokeLinecap="round"
        strokeLinejoin="round"
        className="text-primary/60"
      />
      {/* Puntos en cada valor */}
      {normalizedData.map((point, index) => {
        const x = padding + index * stepX
        const y = height - padding - (point.normalized / 100) * (height - padding * 2)
        return <circle key={index} cx={x} cy={y} r="2" fill="currentColor" className="text-primary" />
      })}
    </svg>
  )
}
