<template>
    <div class="tile">
        <slot></slot>

    </div>
</template>


<style lang="scss" scoped>

@import "~/assets/css/variables.css";

.tile {
    border-radius: 30px;
    background-color: var(--tint);
    padding: 30px 5%; 
    overflow: hidden;
    @media (min-width: 768px) {
        padding: 30px 50px; 
    }

}

</style>