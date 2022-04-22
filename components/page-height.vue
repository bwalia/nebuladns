<template>
    <div class="page-height">
        <slot></slot>
    </div>
</template>

<style lang="css" scoped>

.page-height {
    min-height: calc(100vh - 15em);
}

</style>