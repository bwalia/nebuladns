<template>
    <div class="row">
        <slot></slot>
    </div>
</template>

<style lang="css" scoped>

@import "~/assets/css/variables.css";

.row {
    display: flex;
    flex-wrap: wrap;
    align-items: flex-start;
    justify-content: space-between;
    gap: 4%;
}

</style>