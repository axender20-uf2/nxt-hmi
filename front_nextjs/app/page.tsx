"use client"
import { TemperatureDashboard } from "@/components/temperature-dashboard"

export default function Home() {
  return (
    <div className="min-h-screen bg-background">
      <TemperatureDashboard />
    </div>
  )
}
