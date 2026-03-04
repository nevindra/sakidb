<script lang="ts">
  let {
    name,
    positions = [],
  }: {
    name: string;
    positions?: number[];
  } = $props();

  const segments = $derived.by(() => {
    if (!positions.length) return [{ text: name, highlight: false }];

    const posSet = new Set(positions);
    const result: { text: string; highlight: boolean }[] = [];
    let i = 0;

    while (i < name.length) {
      const isMatch = posSet.has(i);
      let j = i + 1;
      while (j < name.length && posSet.has(j) === isMatch) j++;
      result.push({ text: name.slice(i, j), highlight: isMatch });
      i = j;
    }

    return result;
  });
</script>

<span class="truncate">{#each segments as seg}{#if seg.highlight}<span class="text-primary font-medium">{seg.text}</span>{:else}{seg.text}{/if}{/each}</span>
