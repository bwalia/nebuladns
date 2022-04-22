<template>
    <span class="ticker" :class="{'ticker--negative': !isPositive}">

        <svg style="display: none;">
            <defs>
                <symbol id="ticker" viewBox="0 0 16 16">
                    <title>Ticker</title>
                    <polygon fill="currentcolor" points="8,4 2,13 14,13 "/>
                </symbol>
            </defs>
        </svg>

        <svg><use xlink:href="#ticker"></use></svg>

        <span>{{performance}}<small>%</small></span>

    </span>
</template>

<script>
export default {
    props: ['performance'],
    data () {
        return {
        }
    },
    computed: {
        isPositive() {
            return ((this.performance * 1) > -1) ? true : false;
        }
    }
}
</script>


<style lang="scss" scoped>

@import "~/assets/css/variables.css";
@import "~/assets/css/scale.scss";

.ticker { 
    font-weight: bold;
    display: inline-flex;
    align-items: center;
    color: var(--default); //#20A380;
    background: #fff;
    padding: 0 0.5em;
    border-radius: 5px;

    svg {
        width: 14px;
        height: 14px;
        margin-right: 2px;
    }

    &.ticker--negative {
        color: #E80348;
        svg {
            transform: rotate(180deg) translateY(-1px);
        }
    }
}

</style>