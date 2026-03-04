<script lang="ts">
  let { data, height = 120 }: { data: { label: string; value: number }[]; height?: number } = $props();

  const barGap = 2;
  const paddingBottom = 18;
  const paddingTop = 4;
  const chartHeight = $derived(height - paddingBottom - paddingTop);

  const maxValue = $derived(Math.max(...data.map(d => d.value), 1));
</script>

<svg
  width="100%"
  {height}
  viewBox="0 0 {data.length * 28 + barGap} {height}"
  preserveAspectRatio="xMinYMid meet"
  class="text-muted-foreground"
>
  {#each data as item, i}
    {@const barHeight = (item.value / maxValue) * chartHeight}
    {@const x = i * 28 + barGap}
    {@const y = paddingTop + chartHeight - barHeight}

    <g>
      <title>{item.label}: {item.value.toLocaleString()}</title>
      <rect
        {x}
        {y}
        width="24"
        height={barHeight}
        rx="2"
        class="fill-info/70 hover:fill-info transition-colors"
      />
      <text
        x={x + 12}
        y={height - 2}
        text-anchor="middle"
        class="fill-current text-[8px]"
      >
        {item.label.length > 4 ? item.label.slice(0, 4) : item.label}
      </text>
    </g>
  {/each}
</svg>
