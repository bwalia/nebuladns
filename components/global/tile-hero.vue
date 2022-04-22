<template>
    <div 
        class="hero" :class="{ 'hero--tall': tall }">
        <div class="hero-content">
            <slot></slot>
        </div>

        <div class="hero-image">
            <img src="~/assets/images/mountain.jpg" />
        </div>

    </div>
</template>



<script>
export default {
  props: ['tall']
}
</script>



<style lang="scss" scoped>

@import "~/assets/css/variables.css";

.hero {
    border-radius: 30px;
    position: relative;
    // padding-top: 32%;
    min-height: 16em;
    background: #37495A;
    // background: rgb(55,73,90);
    // background: linear-gradient(126deg, rgba(55,73,90,1) 0%, rgba(170,205,221,0.5) 100%);
    width: 100%;
    overflow: hidden;
    margin-bottom: 2.4em;

    &.hero--tall {
        min-height: 24em;
    }


    .hero-image {
        position: absolute;
        left: 0;
        top: 0;
        width: 100%;
        height: 100%;
        z-index: 1;
        img {
            object-fit: cover;
            object-position: 15% 50%;
            opacity: 0.5;
            width: 100%;
            height: 100%;
        }
    }


    .hero-content {
        position: relative;
        z-index: 2;
        width: calc(100% - 100px);
        color: #fff;
        padding: 30px;
        @media (min-width: 768px) { 
            width: 55%;
            padding: 30px 50px;
        }
    }

}

</style>