<template>
    <div class="grid">
        <slot></slot>
    </div>
</template>

<style lang="scss" scoped>

@import "~/assets/css/variables.css";

.grid {
    display: grid;
    gap: var(--gutter);

    @media (min-width: 768px) {
        grid-auto-flow: column;
        grid-auto-columns: 1fr;
    }
}

</style>