<template>
    <div class="burger-icon">
        <svg style="display: none;">
            <defs>
                <symbol id="line" viewBox="0 0 56 56">
                    <title>Line</title>
                    <line fill="none" stroke="#2C3033" stroke-width="3.5" stroke-linecap="round" x1="2" y1="28" x2="54" y2="28"/>
                </symbol>
            </defs>
        </svg>
        <div class="burger-wrapper">
            <svg class="burger-top"><use xlink:href="#line"></use></svg>
            <svg class="burger-mid"><use xlink:href="#line"></use></svg>
            <svg class="burger-bottom"><use xlink:href="#line"></use></svg>
        </div>
    </div>
</template>

<script>
export default {
    props: ['state'],
    data () {
        return {
        }
    }
}
</script>

<style scoped lang="scss">

@import "~/assets/css/variables.css";

$burger-size: calc( 38px + 4vw );

.burger-icon {
    width: $burger-size;
    height: $burger-size;
    line-height: $burger-size;
    text-align: center;
    padding: 25%;

    .burger-wrapper {
        position: relative;
        width: 100%;
        height: 100%;

        & > * {
            position: absolute;
            top: 0;
            left: 0;
            transition: all 0.33s ease;
            transform-origin: center;
            opacity: 1;
        }

        svg {
            width: 100%;
            height: 100%;
            display: inline-block;
            color: red;
        }
    }
}

.burger-top { transform: translateY(-40%); }
.burger-bottom { transform: translateY(40%); }

.open .burger-icon {
    .burger-top { transform: translateY(0) rotate(45deg); }
    .burger-bottom { transform: translateY(0) rotate(-45deg); }  
    .burger-mid { opacity: 0; }  
}

</style>
