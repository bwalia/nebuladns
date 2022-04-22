<template>
    <div class="logo">
        <nuxt-link to="/">
            <img 
                class="logo" 
                src="/odin-capital-management.png" 
                width="800" 
                height="292" 
                alt="Odin Capital Management" />
        </nuxt-link>
    </div>
</template>

<style lang="scss" scoped>

@import "~/assets/css/variables.css";
@import "~/assets/css/scale.scss";

.logo { 
    flex: 0 0 160px;
    @media (min-width: 768px) {
        flex: 0 0 calc( 8vw + 120px );
    }
}

</style>