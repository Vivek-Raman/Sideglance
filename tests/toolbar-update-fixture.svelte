<script>
  import { onMount } from 'svelte';
  import ViewerToolbar from '../src/viewer/ViewerToolbar.svelte';
  let windowFocused = true;
  onMount(() => {
    const focus = () => (windowFocused = true);
    const blur = () => (windowFocused = false);
    window.addEventListener('focus', focus);
    window.addEventListener('blur', blur);
    return () => {
      window.removeEventListener('focus', focus);
      window.removeEventListener('blur', blur);
    };
  });
</script>

<ViewerToolbar
  settings={null}
  {windowFocused}
  updatesEnabled={true}
  onTheme={async () => {}}
  onTextScale={async () => {}}
  onSettings={async () => {}}
/>
